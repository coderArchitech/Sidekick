//! Web search adapters (Task T-046, PRD KEY-8): Tavily and Brave over a mock
//! HTTP server, key handling and errors.

#![allow(clippy::unwrap_used)]

use std::sync::Arc;

use serde_json::json;
use sidekick_core::ErrorCode;
use sidekick_gateway::retry::RetryPolicy;
use sidekick_gateway::{Gateway, SearchResult};
use sidekick_secrets::{KeyStore, MemoryStore, SecretString};
use wiremock::matchers::{body_partial_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn keys(provider: &str, key: &str) -> Arc<dyn KeyStore> {
    let keys = Arc::new(MemoryStore::default());
    keys.set(provider, &SecretString::from(key.to_owned()))
        .unwrap();
    keys
}

fn gateway(server: &MockServer, keys: Arc<dyn KeyStore>) -> Gateway {
    Gateway::new(keys, false)
        .with_search_base_url("tavily", server.uri())
        .with_search_base_url("brave", server.uri())
        .with_retry_policy(RetryPolicy::immediate())
}

#[tokio::test]
async fn tavily_search_sends_the_key_and_query() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/search"))
        .and(header("authorization", "Bearer tvly-test-key"))
        .and(body_partial_json(
            json!({ "query": "fed rate today", "max_results": 5 }),
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({ "results": [
                { "title": "Rates", "url": "https://x.test/rates", "content": "4.5% as of today." }
            ]})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let gw = gateway(&server, keys("tavily", "tvly-test-key"));
    assert_eq!(
        gw.web_search("tavily", "  fed   rate today ")
            .await
            .unwrap(),
        vec![SearchResult {
            title: "Rates".into(),
            url: "https://x.test/rates".into(),
            snippet: "4.5% as of today.".into(),
        }]
    );
}

#[tokio::test]
async fn brave_search_uses_the_subscription_header() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/res/v1/web/search"))
        .and(header("x-subscription-token", "brave-key"))
        .and(query_param("q", "rust async"))
        .and(query_param("count", "5"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({ "web": { "results": [
            { "title": "Async Rust", "url": "https://rust-lang.org/async", "description": "<strong>Async</strong> book" }
        ]}})))
        .expect(1)
        .mount(&server)
        .await;
    let gw = gateway(&server, keys("brave", "brave-key"));
    let r = gw.web_search("brave", "rust async").await.unwrap();
    assert_eq!(r[0].snippet, "Async book");
}

#[tokio::test]
async fn errors_are_mapped_and_keys_are_required() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/search"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;
    let gw = gateway(&server, keys("tavily", "bad"));
    assert_eq!(
        gw.web_search("tavily", "x").await.unwrap_err().code,
        ErrorCode::KeyInvalid
    );

    let no_keys: Arc<dyn KeyStore> = Arc::new(MemoryStore::default());
    let gw = gateway(&server, no_keys);
    assert_eq!(
        gw.web_search("brave", "x").await.unwrap_err().code,
        ErrorCode::KeyMissing
    );
    assert!(
        gw.web_search("bing", "x").await.is_err(),
        "unknown provider"
    );
    assert!(gw.web_search("tavily", "   ").await.is_err(), "empty query");
}

#[tokio::test]
async fn mock_mode_returns_a_deterministic_result() {
    let gw = Gateway::new(Arc::new(MemoryStore::default()), true);
    let r = gw.web_search("tavily", "pro tier pricing").await.unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].title, "Mock result for pro tier pricing");
}
