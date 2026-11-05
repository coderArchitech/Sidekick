//! Integration tests for sessions and consent (Task T-013).

#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use sidekick_audio::{CaptureHandle, FrameSender, Pacing, StreamKind};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::SecretString;
use sidekick_session::repo::{self, NewSession};
use sidekick_session::{
    AudioFactory, ConsentOption, FixtureAudio, LiveSession, SessionStatus, SttPlan, streams_for,
};
use sidekick_store::Database;
use sidekick_stt::{MockSttProvider, Partial, Segment, SttSink, SttStatus};

fn db() -> Database {
    Database::open_in_memory(&SecretString::from("b".repeat(64))).unwrap()
}

fn new(title: &str) -> NewSession<'_> {
    NewSession {
        title,
        stt_provider: "deepgram",
        llm_provider: "anthropic",
        llm_model: "claude-opus-5-5",
        auto_answer: true,
    }
}

#[test]
fn create_get_and_validate_titles() {
    let db = db();
    let s = repo::create(&db, &new("  Acme QBR  ")).unwrap();
    assert_eq!(
        (s.title.as_str(), s.status, s.consent, s.auto_answer),
        ("Acme QBR", SessionStatus::Draft, None, true)
    );
    assert_eq!(repo::get(&db, &s.id).unwrap(), s);
    assert!(repo::create(&db, &new("   ")).is_err());
    assert!(repo::create(&db, &new(&"x".repeat(201))).is_err());
}

#[test]
fn lifecycle_timestamps() {
    let db = db();
    let s = repo::create(&db, &new("Standup")).unwrap();
    let live = repo::set_status(&db, &s.id, SessionStatus::Live).unwrap();
    let started = live.started_at.unwrap();
    repo::set_status(&db, &s.id, SessionStatus::Paused).unwrap();
    let resumed = repo::set_status(&db, &s.id, SessionStatus::Live).unwrap();
    assert_eq!(resumed.started_at, Some(started), "first start is kept");
    let ended = repo::set_status(&db, &s.id, SessionStatus::Ended).unwrap();
    assert!(ended.ended_at.is_some());
}

#[test]
fn consent_is_recorded_before_start_and_survives_deletion() {
    let db = db();
    let s = repo::create(&db, &new("Client call")).unwrap();
    let s = repo::record_consent(&db, &s.id, ConsentOption::MeOnly).unwrap();
    assert_eq!(s.consent, Some(ConsentOption::MeOnly));
    // Changing your mind before starting: the latest choice wins.
    let s = repo::record_consent(&db, &s.id, ConsentOption::ParticipantsAgreed).unwrap();
    assert_eq!(s.consent, Some(ConsentOption::ParticipantsAgreed));

    repo::set_status(&db, &s.id, SessionStatus::Live).unwrap();
    assert!(
        repo::record_consent(&db, &s.id, ConsentOption::MeOnly).is_err(),
        "only before starting"
    );

    repo::delete(&db, &s.id).unwrap();
    assert!(repo::get(&db, &s.id).is_err());
    let log = repo::consent_log(&db, &s.id).unwrap();
    assert_eq!(
        log.iter().map(|(o, _)| *o).collect::<Vec<_>>(),
        [ConsentOption::MeOnly, ConsentOption::ParticipantsAgreed]
    );
}

#[test]
fn start_without_consent_is_refused() {
    assert_eq!(
        streams_for(None).unwrap_err().code,
        ErrorCode::ConsentRequired
    );
}

#[test]
fn lists_newest_first_with_pagination() {
    let db = db();
    for i in 0..5 {
        repo::create(&db, &new(&format!("Meeting {i}"))).unwrap();
        std::thread::sleep(Duration::from_millis(2));
    }
    let page1 = repo::list(&db, None, Some(2)).unwrap();
    assert_eq!(
        page1
            .items
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        ["Meeting 4", "Meeting 3"]
    );
    let page2 = repo::list(&db, page1.next_cursor.as_deref(), Some(2)).unwrap();
    assert_eq!(
        page2
            .items
            .iter()
            .map(|s| s.title.as_str())
            .collect::<Vec<_>>(),
        ["Meeting 2", "Meeting 1"]
    );
    let page3 = repo::list(&db, page2.next_cursor.as_deref(), Some(2)).unwrap();
    assert_eq!(page3.items.len(), 1);
    assert_eq!(page3.next_cursor, None);
    assert_eq!(
        repo::list(&db, None, None).unwrap().items.len(),
        5,
        "default limit 50"
    );
    assert_eq!(
        repo::list(&db, None, Some(10_000)).unwrap().items.len(),
        5,
        "limit capped at 200"
    );
    assert!(repo::list(&db, Some("garbage"), None).is_err());
}

#[test]
fn segments_are_stored_and_deleted_with_the_session() {
    let db = db();
    let s = repo::create(&db, &new("Review")).unwrap();
    let seg = Segment {
        speaker_label: "speaker_1".into(),
        is_user: false,
        text: "Priya, rate limit?".into(),
        start_ms: 100,
        end_ms: 900,
        confidence: Some(0.9),
    };
    repo::insert_segment(&db, &s.id, &seg).unwrap();
    let count = |db: &Database| {
        db.conn()
            .query_row("SELECT count(*) FROM transcript_segments", [], |r| {
                r.get::<_, i64>(0)
            })
            .unwrap()
    };
    assert_eq!(count(&db), 1);
    repo::delete(&db, &s.id).unwrap();
    assert_eq!(count(&db), 0);
}

// ---- Runner -------------------------------------------------------------------

#[derive(Default)]
struct Sink {
    segments: Mutex<Vec<Segment>>,
}

impl SttSink for Sink {
    fn partial(&self, _: Partial) {}
    fn segment(&self, s: Segment) {
        self.segments.lock().unwrap().push(s);
    }
    fn status(&self, _: StreamKind, _: SttStatus) {}
}

fn write_wav(path: &Path, pattern: &[(bool, f32)]) {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: 16_000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut t = 0usize;
    for &(voice, secs) in pattern {
        for _ in 0..(16_000.0 * secs) as usize {
            let v = if voice {
                (0.4 * (2.0 * std::f32::consts::PI * 300.0 * t as f32 / 16_000.0).sin() * 32_767.0)
                    as i16
            } else {
                0
            };
            w.write_sample(v).unwrap();
            t += 1;
        }
    }
    w.finalize().unwrap();
}

fn plan() -> SttPlan {
    SttPlan {
        provider: Arc::new(MockSttProvider),
        key: None,
        language: "en".into(),
        keyterms: vec![],
    }
}

/// Records which streams were started; system audio can be made to fail.
struct Recording {
    inner: FixtureAudio,
    system_fails: bool,
    started: Mutex<Vec<StreamKind>>,
}

impl AudioFactory for Recording {
    fn start(&self, kind: StreamKind, tx: FrameSender) -> SidekickResult<CaptureHandle> {
        if kind == StreamKind::System && self.system_fails {
            return Err(SidekickError::new(
                ErrorCode::PermissionDenied,
                "System audio needs macOS 14.6.",
            ));
        }
        self.started.lock().unwrap().push(kind);
        self.inner.start(kind, tx)
    }
}

fn recording(dir: &Path, system_fails: bool) -> Recording {
    let wav = dir.join("meeting.wav");
    write_wav(
        &wav,
        &[(true, 0.5), (false, 0.6), (true, 0.5), (false, 0.6)],
    );
    Recording {
        inner: FixtureAudio {
            path: wav,
            pacing: Pacing::Fast,
        },
        system_fails,
        started: Mutex::default(),
    }
}

#[tokio::test]
async fn me_only_starts_the_microphone_only() {
    let dir = tempfile::tempdir().unwrap();
    let audio = recording(dir.path(), false);
    let sink = Arc::new(Sink::default());
    let kinds = streams_for(Some(ConsentOption::MeOnly)).unwrap();
    let live = LiveSession::start(&kinds, &audio, &plan(), sink.clone()).unwrap();
    assert_eq!(live.kinds(), [StreamKind::Mic]);
    tokio::time::sleep(Duration::from_millis(300)).await; // Let the fixture play.
    live.stop().await;
    assert_eq!(
        *audio.started.lock().unwrap(),
        [StreamKind::Mic],
        "system audio never started"
    );
    let segs = sink.segments.lock().unwrap();
    assert_eq!(segs.len(), 2, "two speech bursts in the fixture");
    assert!(segs.iter().all(|s| s.speaker_label == "me" && s.is_user));
}

#[tokio::test]
async fn participants_agreed_starts_both_streams() {
    let dir = tempfile::tempdir().unwrap();
    let audio = recording(dir.path(), false);
    let sink = Arc::new(Sink::default());
    let kinds = streams_for(Some(ConsentOption::ParticipantsAgreed)).unwrap();
    let live = LiveSession::start(&kinds, &audio, &plan(), sink.clone()).unwrap();
    assert_eq!(live.kinds(), [StreamKind::Mic, StreamKind::System]);
    assert!(live.warnings.is_empty());
    tokio::time::sleep(Duration::from_millis(300)).await; // Let the fixture play.
    live.stop().await;
    let labels: Vec<String> = sink
        .segments
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.speaker_label.clone())
        .collect();
    assert!(
        labels.contains(&"me".to_owned()) && labels.contains(&"speaker_1".to_owned()),
        "{labels:?}"
    );
}

#[tokio::test]
async fn missing_system_audio_falls_back_to_the_microphone_with_a_warning() {
    let dir = tempfile::tempdir().unwrap();
    let audio = recording(dir.path(), true);
    let kinds = streams_for(Some(ConsentOption::ParticipantsAgreed)).unwrap();
    let live = LiveSession::start(&kinds, &audio, &plan(), Arc::new(Sink::default())).unwrap();
    assert_eq!(live.kinds(), [StreamKind::Mic]);
    assert_eq!(live.warnings.len(), 1);
    assert!(live.warnings[0].contains("Only your microphone"));
    live.stop().await;
}

struct Failing;
impl AudioFactory for Failing {
    fn start(&self, _: StreamKind, _: FrameSender) -> SidekickResult<CaptureHandle> {
        Err(SidekickError::new(
            ErrorCode::PermissionDenied,
            "No microphone found.",
        ))
    }
}

#[tokio::test]
async fn missing_microphone_fails_the_start() {
    let err = LiveSession::start(
        &[StreamKind::Mic],
        &Failing,
        &plan(),
        Arc::new(Sink::default()),
    )
    .err()
    .unwrap();
    assert_eq!(err.code, ErrorCode::PermissionDenied);
}

#[tokio::test]
async fn pause_and_resume_reach_the_capture() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("long.wav");
    write_wav(&wav, &[(true, 3.0)]);
    let audio = FixtureAudio {
        path: wav,
        pacing: Pacing::RealTime,
    };
    let sink = Arc::new(Sink::default());
    let live = LiveSession::start(&[StreamKind::Mic], &audio, &plan(), sink.clone()).unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    live.pause();
    tokio::time::sleep(Duration::from_millis(600)).await;
    live.resume();
    tokio::time::sleep(Duration::from_millis(300)).await;
    live.stop().await;
    // The pause splits the speech into two bursts: frames are dropped, then resume.
    let segs = sink.segments.lock().unwrap();
    assert!(segs.len() >= 2, "{segs:?}");
    assert!(
        segs[1].start_ms > segs[0].end_ms + 100,
        "gap where the pause was: {segs:?}"
    );
}

#[test]
fn transcript_lines_rename_merge_and_bookmarks() {
    let db = db();
    let s = repo::create(&db, &new("Design review")).unwrap();
    let seg = |label: &str, start: u64, text: &str| Segment {
        speaker_label: label.into(),
        is_user: label == "me",
        text: text.into(),
        start_ms: start,
        end_ms: start + 500,
        confidence: None,
    };
    repo::insert_segment(&db, &s.id, &seg("speaker_2", 2_000, "Second")).unwrap();
    repo::insert_segment(&db, &s.id, &seg("speaker_1", 1_000, "First")).unwrap();
    repo::insert_segment(&db, &s.id, &seg("me", 3_000, "Mine")).unwrap();
    repo::insert_segment(&db, &s.id, &seg("speaker_1", 4_000, "Again")).unwrap();

    let lines = repo::list_segments(&db, &s.id).unwrap();
    assert_eq!(
        lines.iter().map(|l| l.text.as_str()).collect::<Vec<_>>(),
        ["First", "Second", "Mine", "Again"],
        "time order"
    );

    assert_eq!(
        repo::rename_speaker(&db, &s.id, "speaker_1", "Priya").unwrap(),
        2
    );
    // Merge: speaker_2 was really Priya too.
    assert_eq!(
        repo::rename_speaker(&db, &s.id, "speaker_2", "Priya").unwrap(),
        1
    );
    let labels: Vec<String> = repo::list_segments(&db, &s.id)
        .unwrap()
        .into_iter()
        .map(|l| l.speaker_label)
        .collect();
    assert_eq!(labels, ["Priya", "Priya", "me", "Priya"]);
    assert!(repo::rename_speaker(&db, &s.id, "Priya", "  ").is_err());

    repo::add_bookmark(&db, &s.id, 3_500, Some("  pricing  ")).unwrap();
    repo::add_bookmark(&db, &s.id, 1_200, None).unwrap();
    let marks = repo::list_bookmarks(&db, &s.id).unwrap();
    assert_eq!(
        marks
            .iter()
            .map(|b| (b.at_ms, b.label.clone()))
            .collect::<Vec<_>>(),
        [(1_200, None), (3_500, Some("pricing".into()))]
    );
}

/// Records the config each STT run receives.
#[derive(Default)]
struct ConfigSpy(Mutex<Vec<(StreamKind, Vec<String>)>>);

#[async_trait::async_trait]
impl sidekick_stt::SttProvider for ConfigSpy {
    fn id(&self) -> &'static str {
        "spy"
    }
    async fn run(
        &self,
        config: sidekick_stt::SttConfig,
        _: Option<SecretString>,
        mut frames: tokio::sync::mpsc::Receiver<sidekick_audio::AudioFrame>,
        _: Arc<dyn SttSink>,
        _: tokio_util::sync::CancellationToken,
    ) -> SidekickResult<()> {
        self.0
            .lock()
            .unwrap()
            .push((config.kind, config.keyterms.clone()));
        while frames.recv().await.is_some() {}
        Ok(())
    }
}

#[tokio::test]
async fn profile_names_reach_every_stt_stream_as_keyterms() {
    let dir = tempfile::tempdir().unwrap();
    let audio = recording(dir.path(), false);
    let spy = Arc::new(ConfigSpy::default());
    let plan = SttPlan {
        provider: spy.clone(),
        key: None,
        language: "en".into(),
        keyterms: vec!["Nitesh".into(), "Nitu".into()],
    };
    let kinds = streams_for(Some(ConsentOption::ParticipantsAgreed)).unwrap();
    let live = LiveSession::start(&kinds, &audio, &plan, Arc::new(Sink::default())).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    live.stop().await;
    let mut seen = spy.0.lock().unwrap().clone();
    seen.sort_by_key(|(k, _)| format!("{k:?}"));
    assert_eq!(
        seen,
        [
            (
                StreamKind::Mic,
                vec!["Nitesh".to_owned(), "Nitu".to_owned()]
            ),
            (
                StreamKind::System,
                vec!["Nitesh".to_owned(), "Nitu".to_owned()]
            )
        ]
    );
}

#[test]
fn auto_answer_is_per_session() {
    let db = db();
    let a = repo::create(&db, &new("A")).unwrap();
    let b = repo::create(&db, &new("B")).unwrap();
    assert!(
        !repo::set_auto_answer(&db, &a.id, false)
            .unwrap()
            .auto_answer
    );
    assert!(
        repo::get(&db, &b.id).unwrap().auto_answer,
        "other sessions unchanged"
    );
    assert!(repo::set_auto_answer(&db, &a.id, true).unwrap().auto_answer);
}

#[test]
fn answers_are_saved_with_sources_and_rated() {
    use sidekick_session::answers::{self, NewAnswer, QuestionSource, Source};
    let mut db = db();
    let s = repo::create(&db, &new("Q&A")).unwrap();
    let a = NewAnswer {
        session_id: s.id.clone(),
        question: "What's the rate limit?".into(),
        source: QuestionSource::Detected,
        directed: true,
        segment_id: None,
        cue: Some("600 requests/min".into()),
        body_md: "The Pro tier allows 600 requests per minute [1].".into(),
        sources: vec![Source {
            kind: "doc".into(),
            reference: "d1#p14".into(),
            label: "Spec v2, p.14".into(),
        }],
        provider: "anthropic".into(),
        model: "claude-opus-5-5".into(),
        latency_ms: 1840,
        input_tokens: Some(3120),
        output_tokens: Some(142),
    };
    let id = answers::save(&mut db, &a).unwrap();
    let (json, directed): (String, bool) = db
        .conn()
        .query_row("SELECT a.sources_json, q.directed FROM answers a JOIN questions q ON q.id = a.question_id WHERE a.id = ?1", [&id], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap();
    assert_eq!(
        json, r#"[{"type":"doc","ref":"d1#p14","label":"Spec v2, p.14"}]"#,
        "tech doc 4.3 format"
    );
    assert!(directed);
    answers::rate(&db, &id, 1).unwrap();
    answers::rate(&db, &id, 0).unwrap();
    assert!(answers::rate(&db, &id, 2).is_err());
    assert!(answers::rate(&db, "missing", 1).is_err());
    repo::delete(&db, &s.id).unwrap();
    let left: i64 = db
        .conn()
        .query_row("SELECT count(*) FROM answers", [], |r| r.get(0))
        .unwrap();
    assert_eq!(left, 0, "answers go with the session");
}
