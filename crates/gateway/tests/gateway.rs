//! Integration tests for the gateway (Task T-007): Anthropic over a mock HTTP
//! server, retries, key handling, streaming and cancellation.

#![allow(clippy::unwrap_used)] // Test helpers outside #[test] functions.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sidekick_core::ErrorCode;
use sidekick_core::llm::{
    ChatMessage, FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream,
    Usage,
};
use sidekick_core::{SidekickError, SidekickResult};
use sidekick_gateway::providers::anthropic::AnthropicProvider;
use sidekick_gateway::retry::RetryPolicy;
use sidekick_gateway::streams::AnswerSink;
use sidekick_gateway::{Gateway, ProviderKind};
use sidekick_secrets::{ExposeSecret, KeyStore, MemoryStore, SecretString};
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "sk-ant-test-SECRET-1234";

#[derive(Default)]
struct Collect {
    events: Mutex<Vec<String>>,
    finished: tokio::sync::Notify,
}

impl AnswerSink for Collect {
    fn token(&self, id: &str, delta: &str) {
        self.events
            .lock()
            .unwrap()
            .push(format!("{id}:token:{delta}"));
    }
    fn done(&self, id: &str, usage: Usage, finish: FinishReason) {
        self.events.lock().unwrap().push(format!(
            "{id}:done:{}:{}:{finish:?}",
            usage.input_tokens, usage.output_tokens
        ));
        self.finished.notify_one();
    }
    fn error(&self, id: &str, e: &SidekickError) {
        self.events
            .lock()
            .unwrap()
            .push(format!("{id}:error:{:?}", e.code));
        self.finished.notify_one();
    }
}

impl Collect {
    async fn wait(&self) -> Vec<String> {
        tokio::time::timeout(Duration::from_secs(5), self.finished.notified())
            .await
            .unwrap();
        self.events.lock().unwrap().clone()
    }
}

fn request(model: &str) -> LlmRequest {
    LlmRequest {
        model: model.into(),
        system: None,
        messages: vec![ChatMessage::text(Role::User, "What's the rate limit?")],
        max_tokens: 256,
        effort: None,
    }
}

fn sse_body() -> String {
    [
        (r"message_start", r#"{"type":"message_start","message":{"usage":{"input_tokens":12}}}"#),
        ("content_block_start", r#"{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}"#),
        ("content_block_delta", r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"600/min"}}"#),
        ("message_delta", r#"{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":4}}"#),
        ("message_stop", r#"{"type":"message_stop"}"#),
    ]
    .iter()
    .map(|(e, d)| format!("event: {e}\ndata: {d}\n\n"))
    .collect()
}

async fn gateway(server: &MockServer, keys: Arc<dyn KeyStore>) -> Gateway {
    Gateway::new(keys, false)
        .with_llm("anthropic", Arc::new(AnthropicProvider::new(server.uri())))
        .with_retry_policy(RetryPolicy::immediate())
}

fn saved_keys() -> Arc<dyn KeyStore> {
    let keys = Arc::new(MemoryStore::default());
    keys.set("anthropic", &SecretString::from(KEY.to_owned()))
        .unwrap();
    keys
}

#[tokio::test]
async fn streams_an_answer_from_anthropic_with_the_right_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .and(header("x-api-key", KEY))
        .and(header("anthropic-version", "2026-10-21"))
        .and(header("anthropic-beta", "server-side-fallback-2026-07-01"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body(), "text/event-stream"))
        .expect(1)
        .mount(&server)
        .await;
    let gw = gateway(&server, saved_keys()).await;
    let sink = Arc::new(Collect::default());
    gw.start_stream(
        "q1".into(),
        "anthropic",
        request("claude-opus-5-5"),
        sink.clone(),
    )
    .unwrap();
    assert_eq!(
        sink.wait().await,
        vec!["q1:token:600/min", "q1:done:12:4:EndTurn"]
    );
}

#[tokio::test]
async fn retries_429_then_succeeds() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(429).insert_header("retry-after", "0"))
        .up_to_n_times(2)
        .mount(&server)
        .await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(sse_body(), "text/event-stream"))
        .mount(&server)
        .await;
    let gw = gateway(&server, saved_keys()).await;
    let sink = Arc::new(Collect::default());
    gw.start_stream(
        "q2".into(),
        "anthropic",
        request("claude-haiku-5-5"),
        sink.clone(),
    )
    .unwrap();
    assert_eq!(sink.wait().await.last().unwrap(), "q2:done:12:4:EndTurn");
    assert_eq!(server.received_requests().await.unwrap().len(), 3);
}

#[tokio::test]
async fn gives_up_after_three_retries_on_server_errors() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    let gw = gateway(&server, saved_keys()).await;
    let sink = Arc::new(Collect::default());
    gw.start_stream(
        "q3".into(),
        "anthropic",
        request("claude-haiku-5-5"),
        sink.clone(),
    )
    .unwrap();
    assert_eq!(sink.wait().await, vec!["q3:error:ProviderUnavailable"]);
    assert_eq!(server.received_requests().await.unwrap().len(), 4);
}

#[tokio::test]
async fn rejected_key_is_not_saved() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/models"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    let gw = gateway(&server, keys.clone()).await;
    let err = gw
        .set_key("anthropic", SecretString::from(KEY.to_owned()))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::KeyInvalid);
    assert!(!keys.status("anthropic").unwrap().saved);
    assert_eq!(
        server.received_requests().await.unwrap().len(),
        1,
        "401 is not retried"
    );
}

#[tokio::test]
async fn accepted_key_is_saved_and_never_returned() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/models"))
        .and(header("x-api-key", KEY))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": [{ "id": "claude-opus-5-5", "display_name": "Claude Opus 5.5" }]
        })))
        .mount(&server)
        .await;
    let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    let gw = gateway(&server, keys.clone()).await;
    let check = gw
        .set_key("anthropic", SecretString::from(KEY.to_owned()))
        .await
        .unwrap();
    assert!(check.verified);
    let json = serde_json::to_string(&check).unwrap();
    assert!(!json.contains("SECRET"), "{json}");
    assert_eq!(check.key.last4.as_deref(), Some("1234"));
    assert_eq!(keys.get("anthropic").unwrap().unwrap().expose_secret(), KEY);

    let models = gw.list_models("anthropic").await.unwrap();
    assert_eq!(
        models,
        vec![ModelInfo {
            id: "claude-opus-5-5".into(),
            display_name: "Claude Opus 5.5".into()
        }]
    );

    let entries = serde_json::to_string(&gw.providers(ProviderKind::Llm).unwrap()).unwrap();
    assert!(!entries.contains("SECRET"));
    gw.delete_key("anthropic").unwrap();
    assert!(!keys.status("anthropic").unwrap().saved);
}

#[tokio::test]
async fn providers_without_an_adapter_save_unverified() {
    let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    let gw = Gateway::new(keys.clone(), false);
    let check = gw
        .set_key("assemblyai", SecretString::from("aai-key-9999".to_owned()))
        .await
        .unwrap();
    assert!(!check.verified);
    assert!(check.key.saved);
    assert!(
        gw.set_key("not-a-provider", SecretString::from("x".to_owned()))
            .await
            .is_err()
    );
    assert_eq!(
        gw.set_key("deepgram", SecretString::from("  ".to_owned()))
            .await
            .unwrap_err()
            .code,
        ErrorCode::KeyMissing
    );
}

struct Verifier(bool);

#[async_trait::async_trait]
impl sidekick_core::keys::KeyVerifier for Verifier {
    async fn verify(&self, _: &SecretString) -> SidekickResult<()> {
        if self.0 {
            Ok(())
        } else {
            Err(SidekickError::new(ErrorCode::KeyInvalid, "no"))
        }
    }
}

#[tokio::test]
async fn registered_verifiers_check_non_llm_keys() {
    let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    let gw = Gateway::new(keys.clone(), false).with_verifier("deepgram", Arc::new(Verifier(true)));
    assert!(
        gw.set_key("deepgram", SecretString::from("dg-1234".to_owned()))
            .await
            .unwrap()
            .verified
    );
    let gw = Gateway::new(keys.clone(), false).with_verifier("soniox", Arc::new(Verifier(false)));
    let err = gw
        .set_key("soniox", SecretString::from("sx-1".to_owned()))
        .await
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::KeyInvalid);
    assert!(!keys.status("soniox").unwrap().saved);
    let stt = gw.providers(ProviderKind::Stt).unwrap();
    assert!(
        stt.iter()
            .find(|p| p.info.id == "soniox")
            .unwrap()
            .info
            .implemented
    );
}

#[tokio::test]
async fn missing_key_is_reported_before_streaming() {
    let gw = Gateway::new(Arc::new(MemoryStore::default()), false);
    let err = gw
        .start_stream(
            "q4".into(),
            "anthropic",
            request("claude-opus-5-5"),
            Arc::new(Collect::default()),
        )
        .unwrap_err();
    assert_eq!(err.code, ErrorCode::KeyMissing);
}

/// Streams a token every 10 ms, forever.
struct Endless;

#[async_trait::async_trait]
impl LlmProvider for Endless {
    fn id(&self) -> &'static str {
        "endless"
    }
    async fn list_models(&self, _: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        Ok(vec![])
    }
    async fn stream(&self, _: LlmRequest, _: Option<&SecretString>) -> SidekickResult<TokenStream> {
        Ok(Box::pin(futures::stream::unfold((), |()| async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            Some((Ok(StreamEvent::Delta("x".into())), ()))
        })))
    }
}

#[tokio::test]
async fn cancel_stops_tokens_and_reports_cancelled() {
    let gw =
        Gateway::new(Arc::new(MemoryStore::default()), false).with_llm("ollama", Arc::new(Endless));
    let sink = Arc::new(Collect::default());
    gw.start_stream("q5".into(), "ollama", request("any"), sink.clone())
        .unwrap();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(gw.cancel_stream("q5"));
    let events = sink.wait().await;
    assert_eq!(events.last().unwrap(), "q5:error:Cancelled");
    let count = sink.events.lock().unwrap().len();
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert_eq!(
        sink.events.lock().unwrap().len(),
        count,
        "no tokens after cancel"
    );
    assert!(!gw.cancel_stream("q5"), "already finished");
    assert_eq!(gw.active_streams(), 0);
}

#[tokio::test]
async fn records_time_to_first_token() {
    let recorder = Arc::new(sidekick_diagnostics::Recorder::default());
    let gw = Gateway::new(Arc::new(MemoryStore::default()), true).with_recorder(recorder.clone());
    let sink = Arc::new(Collect::default());
    gw.start_stream("q7".into(), "anthropic", request("any"), sink.clone())
        .unwrap();
    sink.wait().await;
    let stats = recorder.snapshot();
    let first = stats
        .iter()
        .find(|s| s.stage == sidekick_diagnostics::Stage::LlmFirstToken)
        .unwrap();
    assert_eq!(first.count, 1);
}

#[tokio::test]
async fn mock_mode_answers_without_keys() {
    let gw = Gateway::new(Arc::new(MemoryStore::default()), true);
    let sink = Arc::new(Collect::default());
    gw.start_stream("q6".into(), "anthropic", request("any"), sink.clone())
        .unwrap();
    let events = sink.wait().await;
    let text: String = events
        .iter()
        .filter_map(|e| e.strip_prefix("q6:token:"))
        .collect();
    assert_eq!(text, "Mock answer to: What's the rate limit?");
    assert!(
        gw.providers(ProviderKind::Llm)
            .unwrap()
            .iter()
            .all(|p| p.info.implemented)
    );
}

mod openai_embeddings {
    use super::*;
    use sidekick_core::embed::EmbeddingProvider;
    use sidekick_gateway::providers::openai::OpenAiEmbedder;
    use wiremock::matchers::body_partial_json;

    fn vector(n: f32) -> Vec<f32> {
        vec![n; 1536]
    }

    #[tokio::test]
    async fn embeds_in_input_order_with_bearer_auth() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/v1/embeddings"))
            .and(header("authorization", "Bearer sk-openai-1"))
            .and(body_partial_json(serde_json::json!({ "model": "text-embedding-3-small", "input": ["a", "b"] })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                // Out of order on purpose: the adapter must sort by index.
                "data": [ { "index": 1, "embedding": vector(2.0) }, { "index": 0, "embedding": vector(1.0) } ]
            })))
            .mount(&server)
            .await;
        let e = OpenAiEmbedder::new(server.uri(), "text-embedding-3-small").unwrap();
        let out = e
            .embed(
                &["a".into(), "b".into()],
                Some(&SecretString::from("sk-openai-1".to_owned())),
            )
            .await
            .unwrap();
        assert_eq!(out.len(), 2);
        assert!((out[0][0] - 1.0).abs() < f32::EPSILON && (out[1][0] - 2.0).abs() < f32::EPSILON);
        assert_eq!(e.dims(), 1536);
    }

    #[tokio::test]
    async fn maps_errors_and_rejects_wrong_counts() {
        let server = MockServer::start().await;
        Mock::given(path("/v1/embeddings"))
            .and(header("authorization", "Bearer bad"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;
        Mock::given(path("/v1/embeddings"))
            .respond_with(ResponseTemplate::new(200).set_body_json(
                serde_json::json!({ "data": [ { "index": 0, "embedding": [0.1, 0.2] } ] }),
            ))
            .mount(&server)
            .await;
        let e = OpenAiEmbedder::new(server.uri(), "text-embedding-3-small").unwrap();
        let bad = e
            .embed(&["a".into()], Some(&SecretString::from("bad".to_owned())))
            .await
            .unwrap_err();
        assert_eq!(bad.code, ErrorCode::KeyInvalid);
        let short = e
            .embed(&["a".into()], Some(&SecretString::from("ok".to_owned())))
            .await
            .unwrap_err();
        assert_eq!(
            short.code,
            ErrorCode::ProviderUnavailable,
            "wrong vector size"
        );
        assert_eq!(
            e.embed(&["a".into()], None).await.unwrap_err().code,
            ErrorCode::KeyMissing
        );
        assert!(
            e.embed(&[], Some(&SecretString::from("x".to_owned())))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn gateway_picks_the_embedder_and_key() {
        let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
        let gw = Gateway::new(keys.clone(), false);
        assert_eq!(
            gw.embedder("openai", None).err().unwrap().code,
            ErrorCode::KeyMissing
        );
        keys.set("openai", &SecretString::from("sk".to_owned()))
            .unwrap();
        let (e, key) = gw.embedder("openai", None).unwrap();
        assert_eq!(
            (e.id(), e.model(), key.is_some()),
            ("openai", "text-embedding-3-small", true)
        );
        assert!(gw.embedder("voyage", None).is_err(), "not built yet");
        let (mock, key) = Gateway::new(keys, true).embedder("openai", None).unwrap();
        assert_eq!((mock.id(), key.is_none()), ("mock", true));
    }
}
