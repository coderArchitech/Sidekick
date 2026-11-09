//! Web search (PRD KEY-8): Tavily and Brave Search adapters, used by agent
//! Skills' `web_search` tool (SKL-4). Results are titles, links and snippets;
//! nothing else from the page is fetched.

use serde::Serialize;
use serde_json::{Value, json};
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};

use super::http::{status_error, transport_error};

/// One web result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Results per query.
pub const MAX_RESULTS: usize = 5;
/// Longest query sent to a provider.
pub const MAX_QUERY: usize = 400;

pub const TAVILY_URL: &str = "https://api.tavily.com";
pub const BRAVE_URL: &str = "https://api.search.brave.com";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchKind {
    Tavily,
    Brave,
}

impl SearchKind {
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "tavily" => Some(Self::Tavily),
            "brave" => Some(Self::Brave),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Tavily => "Tavily",
            Self::Brave => "Brave Search",
        }
    }
}

/// A web search client for one provider.
#[derive(Debug, Clone)]
pub struct WebSearch {
    kind: SearchKind,
    base_url: String,
    http: reqwest::Client,
}

fn clean(text: &str, max: usize) -> String {
    let t: String = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if t.chars().count() > max {
        format!("{}…", t.chars().take(max).collect::<String>())
    } else {
        t
    }
}

fn str_of(v: &Value, key: &str) -> String {
    v[key].as_str().unwrap_or_default().to_owned()
}

/// Reads a provider's reply into results (at most `MAX_RESULTS`).
#[must_use]
pub fn parse(kind: SearchKind, body: &Value) -> Vec<SearchResult> {
    let list = match kind {
        SearchKind::Tavily => &body["results"],
        SearchKind::Brave => &body["web"]["results"],
    };
    let snippet_key = match kind {
        SearchKind::Tavily => "content",
        SearchKind::Brave => "description",
    };
    list.as_array()
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let url = str_of(r, "url");
            // Only web links; never file:, javascript: or relative ones.
            if !(url.starts_with("https://") || url.starts_with("http://")) {
                return None;
            }
            Some(SearchResult {
                title: clean(&str_of(r, "title"), 200),
                url,
                // Brave marks matches with <strong>; keep plain text.
                snippet: clean(
                    &str_of(r, snippet_key)
                        .replace("<strong>", "")
                        .replace("</strong>", ""),
                    500,
                ),
            })
        })
        .take(MAX_RESULTS)
        .collect()
}

impl WebSearch {
    #[must_use]
    pub fn new(kind: SearchKind) -> Self {
        Self::with_base_url(
            kind,
            match kind {
                SearchKind::Tavily => TAVILY_URL,
                SearchKind::Brave => BRAVE_URL,
            },
        )
    }

    /// Points the client at another URL (tests).
    #[must_use]
    pub fn with_base_url(kind: SearchKind, base_url: &str) -> Self {
        Self {
            kind,
            base_url: base_url.trim_end_matches('/').to_owned(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Searches the web for `query`.
    pub async fn search(
        &self,
        query: &str,
        key: &SecretString,
    ) -> SidekickResult<Vec<SearchResult>> {
        let query = clean(query, MAX_QUERY);
        if query.is_empty() {
            return Err(SidekickError::internal("Type something to search for."));
        }
        let name = self.kind.name();
        let req = match self.kind {
            SearchKind::Tavily => self
                .http
                .post(format!("{}/search", self.base_url))
                .bearer_auth(key.expose_secret())
                .json(&json!({
                    "query": query,
                    "max_results": MAX_RESULTS,
                    "search_depth": "basic",
                    "include_answer": false,
                })),
            SearchKind::Brave => self
                .http
                .get(format!("{}/res/v1/web/search", self.base_url))
                .header("X-Subscription-Token", key.expose_secret())
                .header("Accept", "application/json")
                .query(&[("q", query.as_str()), ("count", &MAX_RESULTS.to_string())]),
        };
        let resp = req.send().await.map_err(|e| transport_error(name, &e))?;
        if !resp.status().is_success() {
            let retry = resp
                .headers()
                .get("retry-after")
                .and_then(|h| h.to_str().ok())
                .map(str::to_owned);
            return Err(status_error(name, resp.status().as_u16(), retry.as_deref()));
        }
        let body: Value = resp.json().await.map_err(|_| {
            SidekickError::new(
                ErrorCode::ProviderUnavailable,
                format!("Unexpected reply from {name}."),
            )
        })?;
        Ok(parse(self.kind, &body))
    }
}

#[async_trait::async_trait]
impl KeyVerifier for WebSearch {
    async fn verify(&self, key: &SecretString) -> SidekickResult<()> {
        self.search("Sidekick", key).await.map(|_| ())
    }
}

/// Deterministic results for tests and `SIDEKICK_MOCK_PROVIDERS=1`.
#[must_use]
pub fn mock_results(query: &str) -> Vec<SearchResult> {
    let q = clean(query, MAX_QUERY);
    vec![SearchResult {
        title: format!("Mock result for {q}"),
        url: "https://example.com/mock-search".into(),
        snippet: format!("Mock web snippet about {q}."),
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tavily_and_brave_and_drops_bad_links() {
        let tavily = json!({ "results": [
            { "title": " Fed  rates ", "url": "https://x.test/a", "content": "The rate is 4.5%." },
            { "title": "Bad", "url": "javascript:alert(1)", "content": "x" },
        ]});
        assert_eq!(
            parse(SearchKind::Tavily, &tavily),
            vec![SearchResult {
                title: "Fed rates".into(),
                url: "https://x.test/a".into(),
                snippet: "The rate is 4.5%.".into(),
            }]
        );
        let brave = json!({ "web": { "results": [
            { "title": "Rust", "url": "https://rust-lang.org", "description": "A <strong>fast</strong> language" },
        ]}});
        assert_eq!(
            parse(SearchKind::Brave, &brave)[0].snippet,
            "A fast language"
        );
        assert!(parse(SearchKind::Brave, &json!({})).is_empty());
    }

    #[test]
    fn keeps_at_most_five_results_and_short_snippets() {
        let many: Vec<Value> = (0..9)
            .map(|i| json!({ "title": i.to_string(), "url": format!("https://x.test/{i}"), "content": "y".repeat(900) }))
            .collect();
        let r = parse(SearchKind::Tavily, &json!({ "results": many }));
        assert_eq!(r.len(), MAX_RESULTS);
        assert_eq!(r[0].snippet.chars().count(), 501);
    }
}
