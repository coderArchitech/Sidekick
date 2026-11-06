//! Deepgram adapter against a local WebSocket server (Task T-010).

#![allow(clippy::unwrap_used, clippy::result_large_err)] // The handshake callback's error type is tungstenite's.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use sidekick_audio::{AudioFrame, FRAME_SAMPLES, StreamKind};
use sidekick_core::ErrorCode;
use sidekick_core::keys::KeyVerifier;
use sidekick_secrets::SecretString;
use sidekick_stt::{
    DeepgramProvider, DeepgramVerifier, MockSttProvider, Partial, Segment, SttConfig, SttProvider,
    SttSink, SttStatus,
};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_util::sync::CancellationToken;

const KEY: &str = "dg-test-key";

#[derive(Default)]
struct Sink {
    log: Mutex<Vec<String>>,
    segments: Mutex<Vec<Segment>>,
}

impl SttSink for Sink {
    fn partial(&self, p: Partial) {
        self.log.lock().unwrap().push(format!("partial:{}", p.text));
    }
    fn segment(&self, s: Segment) {
        self.log.lock().unwrap().push(format!("segment:{}", s.text));
        self.segments.lock().unwrap().push(s);
    }
    fn status(&self, _: StreamKind, s: SttStatus) {
        self.log.lock().unwrap().push(format!("status:{s:?}"));
    }
}

impl Sink {
    fn statuses(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.starts_with("status:"))
            .cloned()
            .collect()
    }
}

fn frame(i: u64, speech: bool) -> AudioFrame {
    AudioFrame {
        kind: StreamKind::System,
        start_ms: i * 50,
        samples: vec![if speech { 1000 } else { 0 }; FRAME_SAMPLES],
        speech,
    }
}

fn final_result(text: &str, start: f64, end: f64) -> String {
    serde_json::json!({
        "type": "Results", "is_final": true, "start": start, "duration": end - start,
        "channel": { "alternatives": [{ "transcript": text, "confidence": 0.95,
            "words": [{ "word": text, "punctuated_word": text, "start": start, "end": end, "speaker": 0 }] }] }
    })
    .to_string()
}

/// What the fake server saw on each connection.
#[derive(Default, Debug)]
struct Seen {
    connections: AtomicUsize,
    binary_frames: AtomicUsize,
    texts: Mutex<Vec<String>>,
    auth: Mutex<Vec<String>>,
}

/// Starts a fake Deepgram. `behave(conn_index)` decides: Some(status) rejects the
/// handshake with that HTTP status; None accepts. Accepted connections reply with
/// a final result after `reply_after` binary frames, and drop the first
/// connection after `drop_first_after` frames if set.
async fn server(
    behave: impl Fn(usize) -> Option<u16> + Send + Sync + 'static,
    reply_after: usize,
    drop_first_after: Option<usize>,
) -> (String, Arc<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/v1/listen", listener.local_addr().unwrap());
    let seen = Arc::new(Seen::default());
    let seen2 = Arc::clone(&seen);
    let behave = Arc::new(behave);
    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            let n = seen2.connections.fetch_add(1, Ordering::SeqCst);
            let seen = Arc::clone(&seen2);
            let reject = behave(n);
            tokio::spawn(async move {
                let cb = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
                    let auth = req
                        .headers()
                        .get("authorization")
                        .map(|v| v.to_str().unwrap().to_owned())
                        .unwrap_or_default();
                    seen.auth.lock().unwrap().push(auth);
                    match reject {
                        Some(code) => {
                            let mut r = ErrorResponse::new(None);
                            *r.status_mut() = code.try_into().unwrap();
                            Err(r)
                        }
                        None => Ok(resp),
                    }
                };
                let Ok(mut ws) = tokio_tungstenite::accept_hdr_async(stream, cb).await else {
                    return;
                };
                let mut frames_here = 0;
                while let Some(Ok(msg)) = ws.next().await {
                    match msg {
                        Message::Binary(_) => {
                            frames_here += 1;
                            seen.binary_frames.fetch_add(1, Ordering::SeqCst);
                            if n == 0 && drop_first_after == Some(frames_here) {
                                return; // Drop the TCP connection without a close frame.
                            }
                            if frames_here == reply_after {
                                let t = format!("hello from connection {n}");
                                ws.send(Message::Text(final_result(&t, 0.1, 0.6).into()))
                                    .await
                                    .unwrap();
                            }
                        }
                        Message::Text(t) => {
                            let close = t.contains("CloseStream");
                            seen.texts.lock().unwrap().push(t.to_string());
                            if close {
                                let _ = ws.close(None).await;
                                return;
                            }
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (url, seen)
}

fn config() -> SttConfig {
    let mut c = SttConfig::new(StreamKind::System);
    c.keepalive = Duration::from_millis(300);
    c.reconnect_deadline = Duration::from_secs(3);
    c
}

async fn run(
    url: &str,
    cfg: SttConfig,
    frames: Vec<AudioFrame>,
    hold_open: Duration,
) -> (Arc<Sink>, sidekick_core::SidekickResult<()>) {
    let sink = Arc::new(Sink::default());
    let (tx, rx) = tokio::sync::mpsc::channel(500);
    let provider = DeepgramProvider::new(url);
    let s: Arc<dyn SttSink> = sink.clone();
    let task = tokio::spawn(async move {
        provider
            .run(
                cfg,
                Some(SecretString::from(KEY.to_owned())),
                rx,
                s,
                CancellationToken::new(),
            )
            .await
    });
    for f in frames {
        tx.send(f).await.unwrap();
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    tokio::time::sleep(hold_open).await;
    drop(tx);
    let result = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    (sink, result)
}

#[tokio::test]
async fn transcribes_with_auth_and_meeting_timestamps() {
    let (url, seen) = server(|_| None, 10, None).await;
    // 2 s of silence (skipped), then 1 s of speech.
    let frames: Vec<_> = (0..40)
        .map(|i| frame(i, false))
        .chain((40..60).map(|i| frame(i, true)))
        .collect();
    let (sink, result) = run(&url, config(), frames, Duration::from_millis(100)).await;
    result.unwrap();
    assert_eq!(seen.auth.lock().unwrap()[0], format!("Token {KEY}"));
    assert_eq!(sink.statuses(), ["status:Connecting", "status:Live"]);
    let segs = sink.segments.lock().unwrap();
    assert_eq!(segs[0].text, "hello from connection 0");
    assert_eq!(segs[0].speaker_label, "speaker_1");
    // Provider time 0.1 s maps to meeting time 2.0 s + 0.1 s (the skipped silence is added back).
    assert_eq!((segs[0].start_ms, segs[0].end_ms), (2_100, 2_600));
    assert!(
        seen.texts
            .lock()
            .unwrap()
            .iter()
            .any(|t| t.contains("CloseStream"))
    );
}

#[tokio::test]
async fn silence_is_not_sent_apart_from_a_short_tail() {
    let (url, seen) = server(|_| None, usize::MAX, None).await;
    // 10 speech frames, then 40 silent frames: only the 10-frame tail of silence is sent.
    let frames: Vec<_> = (0..10)
        .map(|i| frame(i, true))
        .chain((10..50).map(|i| frame(i, false)))
        .collect();
    let (_, result) = run(&url, config(), frames, Duration::from_millis(100)).await;
    result.unwrap();
    assert_eq!(seen.binary_frames.load(Ordering::SeqCst), 20);
}

#[tokio::test]
async fn sends_keepalives_during_silence() {
    let (url, seen) = server(|_| None, usize::MAX, None).await;
    let (_, result) = run(
        &url,
        config(),
        vec![frame(0, true)],
        Duration::from_millis(1_200),
    )
    .await;
    result.unwrap();
    let keepalives = seen
        .texts
        .lock()
        .unwrap()
        .iter()
        .filter(|t| t.contains("KeepAlive"))
        .count();
    assert!(keepalives >= 2, "keepalives: {keepalives}");
}

#[tokio::test]
async fn reconnects_and_replays_buffered_audio() {
    let (url, seen) = server(|_| None, 5, Some(5)).await;
    let started = Instant::now();
    let frames: Vec<_> = (0..40).map(|i| frame(i, true)).collect();
    let (sink, result) = run(&url, config(), frames, Duration::from_millis(500)).await;
    result.unwrap();
    assert_eq!(seen.connections.load(Ordering::SeqCst), 2);
    assert_eq!(
        sink.statuses(),
        [
            "status:Connecting",
            "status:Live",
            "status:Reconnecting",
            "status:Live"
        ]
    );
    assert!(
        sink.log
            .lock()
            .unwrap()
            .iter()
            .any(|l| l == "segment:hello from connection 1")
    );
    // Every frame reaches the server: 5 on the first connection + the rest replayed or streamed.
    assert!(
        seen.binary_frames.load(Ordering::SeqCst) >= 40,
        "{}",
        seen.binary_frames.load(Ordering::SeqCst)
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[tokio::test]
async fn rejected_key_fails_without_retrying() {
    let (url, seen) = server(|_| Some(401), 1, None).await;
    let (sink, result) = run(
        &url,
        config(),
        vec![frame(0, true)],
        Duration::from_millis(300),
    )
    .await;
    assert_eq!(result.unwrap_err().code, ErrorCode::KeyInvalid);
    assert_eq!(seen.connections.load(Ordering::SeqCst), 1);
    assert_eq!(sink.statuses().last().unwrap(), "status:Failed");
}

#[tokio::test]
async fn gives_up_after_the_reconnect_deadline() {
    // Nothing listens on this port.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("ws://{}/v1/listen", listener.local_addr().unwrap());
    drop(listener);
    let mut cfg = config();
    cfg.reconnect_deadline = Duration::from_millis(800);
    let sink = Arc::new(Sink::default());
    let (_tx, rx) = tokio::sync::mpsc::channel(10);
    let s: Arc<dyn SttSink> = sink.clone();
    let result = DeepgramProvider::new(url)
        .run(
            cfg,
            Some(SecretString::from(KEY.to_owned())),
            rx,
            s,
            CancellationToken::new(),
        )
        .await;
    assert_eq!(result.unwrap_err().code, ErrorCode::SttDisconnected);
    assert_eq!(sink.statuses().last().unwrap(), "status:Failed");
}

#[tokio::test]
async fn missing_key_is_reported() {
    let (_tx, rx) = tokio::sync::mpsc::channel(1);
    let result = DeepgramProvider::default()
        .run(
            config(),
            None,
            rx,
            Arc::new(Sink::default()),
            CancellationToken::new(),
        )
        .await;
    assert_eq!(result.unwrap_err().code, ErrorCode::KeyMissing);
}

#[tokio::test]
async fn cancel_stops_the_run() {
    let (url, _) = server(|_| None, usize::MAX, None).await;
    let cancel = CancellationToken::new();
    let (_tx, rx) = tokio::sync::mpsc::channel(10);
    let c = cancel.clone();
    let task = tokio::spawn(async move {
        DeepgramProvider::new(url)
            .run(
                config(),
                Some(SecretString::from(KEY.to_owned())),
                rx,
                Arc::new(Sink::default()),
                c,
            )
            .await
    });
    tokio::time::sleep(Duration::from_millis(200)).await;
    cancel.cancel();
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn verifier_checks_the_key() {
    use wiremock::matchers::{header, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};
    let api = MockServer::start().await;
    Mock::given(path("/v1/projects"))
        .and(header("authorization", "Token good"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&api)
        .await;
    Mock::given(path("/v1/projects"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&api)
        .await;
    let v = DeepgramVerifier::new(api.uri());
    v.verify(&SecretString::from("good".to_owned()))
        .await
        .unwrap();
    assert_eq!(
        v.verify(&SecretString::from("bad".to_owned()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::KeyInvalid
    );
}

#[tokio::test]
async fn mock_provider_turns_speech_bursts_into_segments() {
    let sink = Arc::new(Sink::default());
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let s: Arc<dyn SttSink> = sink.clone();
    let task = tokio::spawn(async move {
        MockSttProvider::default()
            .run(
                SttConfig::new(StreamKind::Mic),
                None,
                rx,
                s,
                CancellationToken::new(),
            )
            .await
    });
    for (i, speech) in [true, true, false, false, true, false]
        .into_iter()
        .enumerate()
    {
        let mut f = frame(i as u64, speech);
        f.kind = StreamKind::Mic;
        tx.send(f).await.unwrap();
    }
    drop(tx);
    task.await.unwrap().unwrap();
    let segs = sink.segments.lock().unwrap();
    let got: Vec<(&str, u64, u64, &str)> = segs
        .iter()
        .map(|s| {
            (
                s.text.as_str(),
                s.start_ms,
                s.end_ms,
                s.speaker_label.as_str(),
            )
        })
        .collect();
    assert_eq!(
        got,
        [
            ("Mock speech 1", 0, 100, "me"),
            ("Mock speech 2", 200, 250, "me")
        ]
    );
}

#[tokio::test]
async fn mock_provider_splits_on_a_time_gap() {
    let sink = Arc::new(Sink::default());
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let s: Arc<dyn SttSink> = sink.clone();
    let task = tokio::spawn(async move {
        MockSttProvider::default()
            .run(
                SttConfig::new(StreamKind::Mic),
                None,
                rx,
                s,
                CancellationToken::new(),
            )
            .await
    });
    // Frames 0, 1 then 10, 11: no silent frame, but 400 ms missing (a pause).
    for i in [0u64, 1, 10, 11] {
        let mut f = frame(i, true);
        f.kind = StreamKind::Mic;
        tx.send(f).await.unwrap();
    }
    drop(tx);
    task.await.unwrap().unwrap();
    let spans: Vec<(u64, u64)> = sink
        .segments
        .lock()
        .unwrap()
        .iter()
        .map(|s| (s.start_ms, s.end_ms))
        .collect();
    assert_eq!(spans, [(0, 100), (500, 600)]);
}

#[tokio::test]
async fn mock_provider_follows_a_script_then_falls_back() {
    let sink = Arc::new(Sink::default());
    let (tx, rx) = tokio::sync::mpsc::channel(100);
    let s: Arc<dyn SttSink> = sink.clone();
    let task = tokio::spawn(async move {
        MockSttProvider::with_script(vec!["What's the rate limit for the Pro tier?".into()])
            .run(
                SttConfig::new(StreamKind::Mic),
                None,
                rx,
                s,
                CancellationToken::new(),
            )
            .await
    });
    for i in [0u64, 1, 10, 11] {
        let mut f = frame(i, true);
        f.kind = StreamKind::Mic;
        tx.send(f).await.unwrap();
    }
    drop(tx);
    task.await.unwrap().unwrap();
    let texts: Vec<String> = sink
        .segments
        .lock()
        .unwrap()
        .iter()
        .map(|s| s.text.clone())
        .collect();
    assert_eq!(
        texts,
        ["What's the rate limit for the Pro tier?", "Mock speech 2"]
    );
}
