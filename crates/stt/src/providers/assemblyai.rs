//! AssemblyAI Universal Streaming (v3) adapter.
//!
//! - Auth: `Authorization: <key>` on the WebSocket handshake.
//! - Audio: 16 kHz mono `pcm_s16le`, binary messages.
//! - Text: `Turn` messages; an unfinished turn is a partial, and the
//!   formatted end-of-turn message (`format_turns=true`) is final.
//! - No diarization in streaming: system audio is labelled `speaker_1`.
//! - Finish: `{"type":"Terminate"}`. No keep-alive message.

use std::sync::Arc;

use serde::Deserialize;
use sidekick_audio::AudioFrame;
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{
    Message, client::IntoClientRequest, handshake::client::Request,
};
use tokio_util::sync::CancellationToken;

use super::ws::{Dialect, MessageParser, Parsed, auth_header};
use crate::speakers::Word;
use crate::{SttConfig, SttProvider, SttSink};

pub const DEFAULT_WS_URL: &str = "wss://streaming.assemblyai.com/v3/ws";
pub const DEFAULT_API_URL: &str = "https://api.assemblyai.com";
const NAME: &str = "AssemblyAI";

#[derive(Debug, Clone)]
pub struct AssemblyAiProvider {
    ws_url: String,
}

impl Default for AssemblyAiProvider {
    fn default() -> Self {
        Self::new(DEFAULT_WS_URL)
    }
}

impl AssemblyAiProvider {
    #[must_use]
    pub fn new(ws_url: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
        }
    }

    #[must_use]
    pub fn listen_url(&self, cfg: &SttConfig) -> String {
        let mut url = url::Url::parse(&self.ws_url).unwrap_or_else(|_| {
            #[allow(clippy::expect_used)] // DEFAULT_WS_URL is a valid constant.
            url::Url::parse(DEFAULT_WS_URL).expect("valid default URL")
        });
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("sample_rate", "16000")
                .append_pair("encoding", "pcm_s16le")
                .append_pair("format_turns", "true");
            // English-only model unless another language is asked for.
            if cfg.language != "en" {
                q.append_pair("speech_model", "universal-streaming-multilingual");
            }
            if !cfg.keyterms.is_empty() {
                q.append_pair(
                    "keyterms_prompt",
                    &serde_json::to_string(&cfg.keyterms).unwrap_or_default(),
                );
            }
        }
        url.into()
    }
}

#[derive(Debug, Deserialize)]
struct Msg {
    #[serde(rename = "type", default)]
    kind: String,
    #[serde(default)]
    transcript: String,
    #[serde(default)]
    end_of_turn: bool,
    #[serde(default)]
    turn_is_formatted: bool,
    end_of_turn_confidence: Option<f32>,
    #[serde(default)]
    words: Vec<AaiWord>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AaiWord {
    start: u64,
    end: u64,
}

/// Parses one message.
pub fn parse(text: &str) -> SidekickResult<Parsed> {
    let Ok(msg) = serde_json::from_str::<Msg>(text) else {
        return Ok(Parsed::Ignore);
    };
    if let Some(err) = msg.error {
        let lower = err.to_lowercase();
        let code = if lower.contains("auth") || lower.contains("api key") {
            ErrorCode::KeyInvalid
        } else {
            ErrorCode::SttDisconnected
        };
        return Err(SidekickError::new(
            code,
            format!("{NAME} reported an error."),
        ));
    }
    if msg.kind != "Turn" || msg.transcript.trim().is_empty() {
        return Ok(Parsed::Ignore); // Begin, Termination, empty turns
    }
    if !msg.end_of_turn {
        return Ok(Parsed::Partial(msg.transcript));
    }
    if !msg.turn_is_formatted {
        // The formatted version of this turn follows.
        return Ok(Parsed::Ignore);
    }
    let start_ms = msg.words.first().map_or(0, |w| w.start);
    let end_ms = msg.words.last().map_or(start_ms, |w| w.end);
    Ok(Parsed::Final {
        words: vec![Word {
            text: msg.transcript,
            start_ms,
            end_ms,
            speaker: None,
        }],
        confidence: msg.end_of_turn_confidence,
    })
}

struct Parser;

impl MessageParser for Parser {
    fn parse(&mut self, text: &str) -> SidekickResult<Vec<Parsed>> {
        parse(text).map(|p| vec![p])
    }
}

impl Dialect for AssemblyAiProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn request(&self, cfg: &SttConfig, key: &SecretString) -> SidekickResult<Request> {
        let mut req = self
            .listen_url(cfg)
            .into_client_request()
            .map_err(|_| SidekickError::internal("Invalid AssemblyAI URL."))?;
        req.headers_mut()
            .insert("Authorization", auth_header(key.expose_secret(), NAME)?);
        Ok(req)
    }

    fn parser(&self, _cfg: &SttConfig) -> Box<dyn MessageParser> {
        Box::new(Parser)
    }

    fn keepalive(&self) -> Option<Message> {
        None
    }

    fn finish(&self) -> Option<Message> {
        Some(Message::Text(r#"{"type":"Terminate"}"#.into()))
    }
}

#[async_trait::async_trait]
impl SttProvider for AssemblyAiProvider {
    fn id(&self) -> &'static str {
        "assemblyai"
    }

    async fn run(
        &self,
        config: SttConfig,
        key: Option<SecretString>,
        frames: Receiver<AudioFrame>,
        sink: Arc<dyn SttSink>,
        cancel: CancellationToken,
    ) -> SidekickResult<()> {
        let key = key.ok_or_else(|| {
            SidekickError::new(
                ErrorCode::KeyMissing,
                "Add your AssemblyAI API key in Settings.",
            )
        })?;
        super::ws::run(self, config, &key, frames, sink, cancel).await
    }
}

/// Checks a key with `GET /v2/transcript?limit=1`.
#[derive(Debug, Clone)]
pub struct AssemblyAiVerifier {
    api_url: String,
    http: reqwest::Client,
}

impl Default for AssemblyAiVerifier {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL)
    }
}

impl AssemblyAiVerifier {
    #[must_use]
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl KeyVerifier for AssemblyAiVerifier {
    async fn verify(&self, key: &SecretString) -> SidekickResult<()> {
        let resp = self
            .http
            .get(format!("{}/v2/transcript?limit=1", self.api_url))
            .header("Authorization", key.expose_secret())
            .send()
            .await
            .map_err(|_| {
                SidekickError::new(ErrorCode::ProviderUnavailable, "Couldn't reach AssemblyAI.")
            })?;
        super::rest_status(NAME, resp.status().as_u16())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use sidekick_audio::StreamKind;

    #[test]
    fn builds_the_url_with_keyterms_and_language() {
        let mut cfg = SttConfig::new(StreamKind::System);
        cfg.keyterms = vec!["Nitesh".into()];
        let url = AssemblyAiProvider::default().listen_url(&cfg);
        assert!(url.starts_with("wss://streaming.assemblyai.com/v3/ws?sample_rate=16000&encoding=pcm_s16le&format_turns=true"));
        assert!(url.contains("keyterms_prompt=%5B%22Nitesh%22%5D"));
        assert!(!url.contains("speech_model"));
        cfg.language = "es".into();
        assert!(
            AssemblyAiProvider::default()
                .listen_url(&cfg)
                .contains("speech_model=universal-streaming-multilingual")
        );
    }

    #[test]
    fn parses_a_recorded_session() {
        let fixture = include_str!("../../tests/fixtures/assemblyai/session.jsonl");
        let parsed: Vec<Parsed> = fixture
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| parse(l).unwrap())
            .collect();
        assert_eq!(parsed[0], Parsed::Ignore, "Begin");
        assert_eq!(parsed[1], Parsed::Partial("priya what's the".into()));
        assert_eq!(parsed[2], Parsed::Ignore, "unformatted end of turn");
        assert_eq!(
            parsed[3],
            Parsed::Final {
                words: vec![Word {
                    text: "Priya, what's the rate limit?".into(),
                    start_ms: 1280,
                    end_ms: 3020,
                    speaker: None
                }],
                confidence: Some(0.92)
            }
        );
        assert_eq!(parsed[4], Parsed::Ignore, "Termination");
    }

    #[test]
    fn maps_errors() {
        assert_eq!(
            parse(r#"{"error":"Unauthorized connection: Invalid API key"}"#)
                .unwrap_err()
                .code,
            ErrorCode::KeyInvalid
        );
        assert_eq!(
            parse(r#"{"error":"Audio too short"}"#).unwrap_err().code,
            ErrorCode::SttDisconnected
        );
        assert_eq!(parse("nope").unwrap(), Parsed::Ignore);
    }
}
