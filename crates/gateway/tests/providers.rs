//! Integration tests for the OpenAI, Gemini and Ollama adapters (Task T-026)
//! against a mock HTTP server: happy path, 401, 429 and 500 for each, through
//! the gateway (keys, retries, stream events).

#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use sidekick_core::llm::{ChatMessage, FinishReason, LlmRequest, Role, Usage};
use sidekick_core::{ErrorCode, SidekickError};
use sidekick_gateway::Gateway;
use sidekick_gateway::providers::gemini::GeminiProvider;
use sidekick_gateway::providers::ollama::OllamaProvider;
use sidekick_gateway::providers::openai_chat::OpenAiProvider;
use sidekick_gateway::retry::RetryPolicy;
use sidekick_gateway::streams::AnswerSink;
use sidekick_secrets::{KeyStore, MemoryStore, SecretString};
use wiremock::matchers::{body_partial_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const KEY: &str = "provider-test-SECRET-5678";

#[derive(Default)]
struct Collect {
    text: Mutex<String>,
    end: Mutex<Option<Result<(Usage, FinishReason), ErrorCode>>>,
    finished: tokio::sync::Notify,
}

impl AnswerSink for Collect {
    fn token(&self, _id: &str, delta: &str) {
        self.text.lock().unwrap().push_str(delta);
    }
    fn done(&self, _id: &str, usage: Usage, finish: FinishReason) {
        *self.end.lock().unwrap() = Some(Ok((usage, finish)));
        self.finished.notify_one();
    }
    fn error(&self, _id: &str, e: &SidekickError) {
        *self.end.lock().unwrap() = Some(Err(e.code));
        self.finished.notify_one();
    }
}

impl Collect {
    async fn result(&self) -> (String, Result<(Usage, FinishReason), ErrorCode>) {
        tokio::time::timeout(Duration::from_secs(5), self.finished.notified())
            .await
            .unwrap();
        (
            self.text.lock().unwrap().clone(),
            self.end.lock().unwrap().unwrap(),
        )
    }
}

fn request(model: &str) -> LlmRequest {
    LlmRequest {
        model: model.into(),
        system: Some("Be brief.".into()),
        messages: vec![ChatMessage::text(Role::User, "What's the rate limit?")],
        max_tokens: 256,
        effort: None,
    }
}

struct Case {
    id: &'static str,
    /// Request path for streaming.
    stream_path: &'static str,
    fixture: &'static str,
    content_type: &'static str,
    usage: Usage,
}

fn cases() -> [Case; 3] {
    [
        Case {
            id: "openai",
            stream_path: "/v1/chat/completions",
            fixture: include_str!("fixtures/openai/stream.sse"),
            content_type: "text/event-stream",
            usage: Usage {
                input_tokens: 812,
                output_tokens: 14,
            },
        },
        Case {
            id: "gemini",
            stream_path: "/v1beta/models/gemini-2.5-flash:streamGenerateContent",
            fixture: include_str!("fixtures/gemini/stream.sse"),
            content_type: "text/event-stream",
            usage: Usage {
                input_tokens: 790,
                output_tokens: 15,
            },
        },
        Case {
            id: "ollama",
            stream_path: "/api/chat",
            fixture: include_str!("fixtures/ollama/chat.ndjson"),
            content_type: "application/x-ndjson",
            usage: Usage {
                input_tokens: 640,
                output_tokens: 13,
            },
        },
    ]
}

fn model_for(id: &str) -> &'static str {
    match id {
        "openai" => "gpt-5-mini",
        "gemini" => "gemini-2.5-flash",
        _ => "llama3.2",
    }
}

fn gateway(server: &MockServer) -> Gateway {
    let keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    keys.set("openai", &SecretString::from(KEY.to_owned()))
        .unwrap();
    keys.set("gemini", &SecretString::from(KEY.to_owned()))
        .unwrap();
    Gateway::new(keys, false)
        .with_llm("openai", Arc::new(OpenAiProvider::new(server.uri())))
        .with_llm("gemini", Arc::new(GeminiProvider::new(server.uri())))
        .with_llm("ollama", Arc::new(OllamaProvider::new(server.uri())))
        .with_retry_policy(RetryPolicy::immediate())
}

async fn run(gw: &Gateway, id: &str) -> (String, Result<(Usage, FinishReason), ErrorCode>) {
    let sink = Arc::new(Collect::default());
    gw.start_stream("r1".into(), id, request(model_for(id)), sink.clone())
        .unwrap();
    sink.result().await
}

#[tokio::test]
async fn each_provider_streams_an_answer_with_its_auth() {
    for c in cases() {
        let server = MockServer::start().await;
        let auth = match c.id {
            "openai" => Some(("authorization", format!("Bearer {KEY}"))),
            "gemini" => Some(("x-goog-api-key", KEY.to_owned())),
            _ => None,
        };
        let mut mock = Mock::given(method("POST")).and(path(c.stream_path));
        if let Some((name, value)) = auth {
            mock = mock.and(header(name, value.as_str()));
        }
        mock.respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", c.content_type)
                .set_body_string(c.fixture),
        )
        .expect(1)
        .mount(&server)
        .await;
        let (text, end) = run(&gateway(&server), c.id).await;
        assert_eq!(
            text, "The Pro tier allows 600 requests per minute [1].",
            "{}",
            c.id
        );
        assert_eq!(end, Ok((c.usage, FinishReason::EndTurn)), "{}", c.id);
    }
}

#[tokio::test]
async fn http_errors_map_to_stable_codes() {
    for c in cases() {
        for (status, code, calls) in [
            (401, ErrorCode::KeyInvalid, 1),
            (429, ErrorCode::RateLimited, 4),
            (500, ErrorCode::ProviderUnavailable, 4),
        ] {
            let server = MockServer::start().await;
            Mock::given(method("POST"))
                .and(path(c.stream_path))
                .respond_with(
                    ResponseTemplate::new(status)
                        .set_body_string("{\"error\":\"echoed prompt text\"}"),
                )
                .expect(calls) // 1 try + 3 retries for retryable codes
                .mount(&server)
                .await;
            let (text, end) = run(&gateway(&server), c.id).await;
            assert!(text.is_empty());
            assert_eq!(end, Err(code), "{} HTTP {status}", c.id);
        }
    }
}

#[tokio::test]
async fn gemini_bad_key_400_is_key_invalid() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(400).set_body_string(
            r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"reason":"API_KEY_INVALID"}]}}"#,
        ))
        .mount(&server)
        .await;
    let (_, end) = run(&gateway(&server), "gemini").await;
    assert_eq!(end, Err(ErrorCode::KeyInvalid));
}

#[tokio::test]
async fn list_models_for_each_provider() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"data":[{"id":"gpt-5"},{"id":"text-embedding-3-small"},{"id":"gpt-5-mini"},{"id":"tts-1"}]}"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1beta/models"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"models":[{"name":"models/gemini-2.5-pro","displayName":"Gemini 2.5 Pro","supportedGenerationMethods":["generateContent","countTokens"]},{"name":"models/text-embedding-004","supportedGenerationMethods":["embedContent"]}]}"#,
        ))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(
                r#"{"models":[{"name":"llama3.2:latest"},{"name":"llava:latest"}]}"#,
            ),
        )
        .mount(&server)
        .await;
    let gw = gateway(&server);
    let ids =
        |v: Vec<sidekick_core::llm::ModelInfo>| v.into_iter().map(|m| m.id).collect::<Vec<_>>();
    assert_eq!(
        ids(gw.list_models("openai").await.unwrap()),
        ["gpt-5", "gpt-5-mini"]
    );
    let gemini = gw.list_models("gemini").await.unwrap();
    assert_eq!(gemini.len(), 1);
    assert_eq!(
        (gemini[0].id.as_str(), gemini[0].display_name.as_str()),
        ("gemini-2.5-pro", "Gemini 2.5 Pro")
    );
    assert_eq!(
        ids(gw.list_models("ollama").await.unwrap()),
        ["llama3.2:latest", "llava:latest"]
    );
}

#[tokio::test]
async fn ollama_needs_no_key_and_resolves_the_default_model() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/tags"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(r#"{"models":[{"name":"qwen3:8b"}]}"#),
        )
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .and(body_partial_json(
            serde_json::json!({ "model": "qwen3:8b" }),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(include_str!("fixtures/ollama/chat.ndjson")),
        )
        .expect(1)
        .mount(&server)
        .await;
    // No keys at all.
    let gw = Gateway::new(Arc::new(MemoryStore::default()), false)
        .with_llm("ollama", Arc::new(OllamaProvider::new(server.uri())))
        .with_retry_policy(RetryPolicy::immediate());
    let sink = Arc::new(Collect::default());
    gw.start_stream("r1".into(), "ollama", request("default"), sink.clone())
        .unwrap();
    let (text, end) = sink.result().await;
    assert!(text.starts_with("The Pro tier"));
    assert!(end.is_ok());
}

#[tokio::test]
async fn ollama_missing_model_says_how_to_pull_it() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/chat"))
        .respond_with(
            ResponseTemplate::new(404).set_body_string(
                r#"{"error":"model \"mistral\" not found, try pulling it first"}"#,
            ),
        )
        .mount(&server)
        .await;
    let p: Arc<dyn sidekick_core::llm::LlmProvider> = Arc::new(OllamaProvider::new(server.uri()));
    let err = match p.stream(request("mistral"), None).await {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert_eq!(err.code, ErrorCode::Internal);
    assert!(
        err.message.contains("ollama pull mistral"),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn keys_never_appear_in_error_messages() {
    for c in cases() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(ResponseTemplate::new(401).set_body_string(format!("bad key {KEY}")))
            .mount(&server)
            .await;
        let sink = Arc::new(Collect::default());
        let gw = gateway(&server);
        gw.start_stream("r".into(), c.id, request(model_for(c.id)), sink.clone())
            .unwrap();
        let _ = sink.result().await;
        let err = gw.list_models(c.id).await.unwrap_err();
        assert!(!err.message.contains(KEY), "{}", c.id);
    }
}
