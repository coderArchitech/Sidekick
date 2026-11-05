//! AssemblyAI and Soniox adapters end to end against a local WebSocket server
//! (Task T-027): auth, audio, results, speaker labels and the finish message,
//! through the shared streaming engine.

#![allow(clippy::unwrap_used, clippy::result_large_err)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::{SinkExt, StreamExt};
use sidekick_audio::{AudioFrame, FRAME_SAMPLES, StreamKind};
use sidekick_core::ErrorCode;
use sidekick_secrets::SecretString;
use sidekick_stt::providers::assemblyai::AssemblyAiProvider;
use sidekick_stt::providers::soniox::SonioxProvider;
use sidekick_stt::{Partial, Segment, SttConfig, SttProvider, SttSink, SttStatus};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::handshake::server::{ErrorResponse, Request, Response};
use tokio_util::sync::CancellationToken;

const KEY: &str = "stt-test-key";

#[derive(Default)]
struct Sink {
    segments: Mutex<Vec<Segment>>,
    partials: Mutex<Vec<String>>,
    statuses: Mutex<Vec<SttStatus>>,
}

impl SttSink for Sink {
    fn partial(&self, p: Partial) {
        self.partials.lock().unwrap().push(p.text);
    }
    fn segment(&self, s: Segment) {
        self.segments.lock().unwrap().push(s);
    }
    fn status(&self, _: StreamKind, s: SttStatus) {
        self.statuses.lock().unwrap().push(s);
    }
}

#[derive(Default)]
struct Seen {
    auth: Mutex<Option<String>>,
    texts: Mutex<Vec<String>>,
    binary: Mutex<usize>,
}

/// A fake provider: after `reply_after` audio frames it sends `replies`, and
/// it closes when it gets `finish` (a text message equal to it).
async fn server(
    replies: Vec<String>,
    reply_after: usize,
    finish: &'static str,
    reject: Option<u16>,
) -> (String, Arc<Seen>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}/ws", listener.local_addr().unwrap());
    let seen = Arc::new(Seen::default());
    let seen2 = Arc::clone(&seen);
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let seen = Arc::clone(&seen2);
            let replies = replies.clone();
            tokio::spawn(async move {
                let cb = |req: &Request, resp: Response| -> Result<Response, ErrorResponse> {
                    *seen.auth.lock().unwrap() = req
                        .headers()
                        .get("authorization")
                        .map(|v| v.to_str().unwrap().to_owned());
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
                let mut n = 0;
                while let Some(Ok(msg)) = ws.next().await {
                    match msg {
                        Message::Binary(_) => {
                            n += 1;
                            *seen.binary.lock().unwrap() += 1;
                            if n == reply_after {
                                for r in &replies {
                                    ws.send(Message::Text(r.clone().into())).await.unwrap();
                                }
                            }
                        }
                        Message::Text(t) => {
                            let done = t.as_str() == finish;
                            seen.texts.lock().unwrap().push(t.to_string());
                            if done {
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

fn frames(n: u64) -> Vec<AudioFrame> {
    (0..n)
        .map(|i| AudioFrame {
            kind: StreamKind::System,
            start_ms: 1_000 + i * 50,
            samples: vec![1000; FRAME_SAMPLES],
            speech: true,
        })
        .collect()
}

async fn run(
    provider: impl SttProvider + 'static,
    cfg: SttConfig,
) -> (Arc<Sink>, sidekick_core::SidekickResult<()>) {
    let sink = Arc::new(Sink::default());
    let (tx, rx) = tokio::sync::mpsc::channel(100);
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
    for f in frames(20) {
        tx.send(f).await.unwrap();
    }
    tokio::time::sleep(Duration::from_millis(200)).await;
    drop(tx);
    let r = tokio::time::timeout(Duration::from_secs(10), task)
        .await
        .unwrap()
        .unwrap();
    (sink, r)
}

fn config() -> SttConfig {
    let mut c = SttConfig::new(StreamKind::System);
    c.reconnect_deadline = Duration::from_secs(2);
    c
}

#[tokio::test]
async fn assemblyai_transcribes_with_header_auth_and_terminates() {
    let fixture: Vec<String> = include_str!("fixtures/assemblyai/session.jsonl")
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let (url, seen) = server(fixture, 10, r#"{"type":"Terminate"}"#, None).await;
    let (sink, result) = run(AssemblyAiProvider::new(url), config()).await;
    result.unwrap();
    assert_eq!(
        seen.auth.lock().unwrap().as_deref(),
        Some(KEY),
        "raw key, no scheme"
    );
    assert_eq!(*seen.binary.lock().unwrap(), 20);
    assert!(
        seen.texts
            .lock()
            .unwrap()
            .iter()
            .any(|t| t.contains("Terminate"))
    );
    assert_eq!(
        sink.partials.lock().unwrap().as_slice(),
        ["priya what's the"]
    );
    let segs = sink.segments.lock().unwrap();
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].text, "Priya, what's the rate limit?");
    assert_eq!(segs[0].speaker_label, "speaker_1");
    // Provider times map back onto meeting time (audio started at 1 s).
    assert_eq!(segs[0].start_ms, 1_000 + 1_280);
}

#[tokio::test]
async fn soniox_sends_config_first_and_labels_speakers() {
    let fixture: Vec<String> = include_str!("fixtures/soniox/session.jsonl")
        .lines()
        .filter(|l| !l.is_empty())
        .map(str::to_owned)
        .collect();
    let (url, seen) = server(fixture, 10, "", None).await;
    let mut cfg = config();
    cfg.language = "multi".into();
    let (sink, result) = run(SonioxProvider::new(url), cfg).await;
    result.unwrap();
    assert_eq!(
        *seen.auth.lock().unwrap(),
        None,
        "the key isn't in a header"
    );
    let texts = seen.texts.lock().unwrap();
    let first: serde_json::Value = serde_json::from_str(&texts[0]).unwrap();
    assert_eq!(first["api_key"], KEY);
    assert_eq!(first["language_hints"], serde_json::json!(["en", "hi"]));
    assert_eq!(texts.last().unwrap(), "", "empty message ends the stream");
    let segs = sink.segments.lock().unwrap();
    let got: Vec<_> = segs
        .iter()
        .map(|s| (s.speaker_label.as_str(), s.text.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("speaker_1", "Priya, what's the limit?"),
            ("speaker_2", "Six hundred.")
        ]
    );
}

#[tokio::test]
async fn rejected_keys_fail_without_retrying() {
    for provider in ["assemblyai", "soniox"] {
        let (url, _) = server(vec![], 1, "", Some(401)).await;
        let (sink, result) = match provider {
            "assemblyai" => run(AssemblyAiProvider::new(url), config()).await,
            _ => run(SonioxProvider::new(url), config()).await,
        };
        assert_eq!(
            result.unwrap_err().code,
            ErrorCode::KeyInvalid,
            "{provider}"
        );
        assert_eq!(
            sink.statuses.lock().unwrap().last(),
            Some(&SttStatus::Failed)
        );
    }
}

#[tokio::test]
async fn soniox_in_stream_auth_error_stops_the_run() {
    let (url, _) = server(
        vec![r#"{"error_code":401,"error_message":"Invalid API key."}"#.into()],
        1,
        "",
        None,
    )
    .await;
    let (_, result) = run(SonioxProvider::new(url), config()).await;
    assert_eq!(result.unwrap_err().code, ErrorCode::KeyInvalid);
}

mod model_download {
    use sidekick_stt::models::{Digest, download};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    async fn serve(body: &'static [u8]) -> MockServer {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ggml-test.bin"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body))
            .mount(&server)
            .await;
        server
    }

    #[tokio::test]
    async fn keeps_a_file_whose_digest_matches() {
        let server = serve(b"model weights").await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("models/ggml-test.bin");
        let mut seen = Vec::new();
        let real = {
            use sha2::Digest as _;
            hex::encode(sha2::Sha256::digest(b"model weights"))
        };
        let real: &'static str = Box::leak(real.into_boxed_str());
        download(
            &reqwest::Client::new(),
            &format!("{}/ggml-test.bin", server.uri()),
            &dest,
            Digest::Sha256(real),
            |done, total| seen.push((done, total)),
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), b"model weights");
        assert_eq!(seen.last(), Some(&(13, Some(13))));
        assert!(!dest.with_extension("part").exists());
    }

    #[tokio::test]
    async fn rejects_a_file_with_the_wrong_sha256_and_leaves_nothing() {
        let server = serve(b"tampered weights").await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("ggml-test.bin");
        let err = download(
            &reqwest::Client::new(),
            &format!("{}/ggml-test.bin", server.uri()),
            &dest,
            Digest::Sha256("0000000000000000000000000000000000000000000000000000000000000000"),
            |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("checksum"), "{}", err.message);
        assert!(!dest.exists());
        assert!(!dest.with_extension("part").exists());
    }

    #[tokio::test]
    async fn rejects_a_wrong_sha1_too() {
        let server = serve(b"x").await;
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("ggml-test.bin");
        assert!(
            download(
                &reqwest::Client::new(),
                &format!("{}/ggml-test.bin", server.uri()),
                &dest,
                Digest::Sha1("c78c86eb1a8faa21b369bcd33207cc90d64ae9df"),
                |_, _| {},
            )
            .await
            .is_err()
        );
        assert!(!dest.exists());
    }

    #[tokio::test]
    async fn http_errors_are_reported() {
        let server = MockServer::start().await;
        let dir = tempfile::tempdir().unwrap();
        let err = download(
            &reqwest::Client::new(),
            &format!("{}/missing.bin", server.uri()),
            &dir.path().join("m.bin"),
            Digest::Sha1("00"),
            |_, _| {},
        )
        .await
        .unwrap_err();
        assert!(err.message.contains("HTTP 404"));
    }
}
