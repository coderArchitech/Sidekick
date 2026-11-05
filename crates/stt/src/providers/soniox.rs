//! Soniox real-time adapter.
//!
//! - Auth: the key goes in the first (config) message, not a header.
//! - Audio: 16 kHz mono `pcm_s16le`, binary messages.
//! - Text: tokens (word pieces) arrive with `is_final`; final tokens are
//!   collected until an `<end>` token (endpoint) and then become one result.
//!   Non-final tokens are the partial.
//! - Diarization on system audio (`speaker` "1", "2", ...).
//! - `multi` language hints English and Hindi, for Hinglish (PRD 11).
//! - Keep-alive: `{"type":"keepalive"}`. Finish: an empty message.

use std::sync::Arc;

use serde::Deserialize;
use serde_json::json;
use sidekick_audio::{AudioFrame, StreamKind};
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{
    Message, client::IntoClientRequest, handshake::client::Request,
};
use tokio_util::sync::CancellationToken;

use super::ws::{Dialect, MessageParser, Parsed};
use crate::speakers::Word;
use crate::{SttConfig, SttProvider, SttSink};

pub const DEFAULT_WS_URL: &str = "wss://stt-rt.soniox.com/transcribe-websocket";
pub const DEFAULT_API_URL: &str = "https://api.soniox.com";
pub const MODEL: &str = "stt-rt-v3";
const NAME: &str = "Soniox";
const END: &str = "<end>";

#[derive(Debug, Clone)]
pub struct SonioxProvider {
    ws_url: String,
}

impl Default for SonioxProvider {
    fn default() -> Self {
        Self::new(DEFAULT_WS_URL)
    }
}

impl SonioxProvider {
    #[must_use]
    pub fn new(ws_url: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
        }
    }
}

/// The first message: settings and the key.
#[must_use]
pub fn config_message(cfg: &SttConfig, key: &SecretString) -> String {
    let hints: Vec<&str> = match cfg.language.as_str() {
        "multi" => vec!["en", "hi"],
        l => vec![l.split('-').next().unwrap_or("en")],
    };
    let mut c = json!({
        "api_key": key.expose_secret(),
        "model": MODEL,
        "audio_format": "pcm_s16le",
        "sample_rate": 16000,
        "num_channels": 1,
        "language_hints": hints,
        "enable_endpoint_detection": true,
        "enable_speaker_diarization": cfg.kind == StreamKind::System,
    });
    if !cfg.keyterms.is_empty() {
        c["context"] = json!({ "terms": cfg.keyterms });
    }
    c.to_string()
}

#[derive(Debug, Deserialize)]
struct Msg {
    #[serde(default)]
    tokens: Vec<Token>,
    error_code: Option<u16>,
    #[serde(default)]
    finished: bool,
}

#[derive(Debug, Clone, Deserialize)]
struct Token {
    text: String,
    #[serde(default)]
    start_ms: u64,
    #[serde(default)]
    end_ms: u64,
    confidence: Option<f32>,
    #[serde(default)]
    is_final: bool,
    speaker: Option<String>,
}

/// Collects final tokens into words until an endpoint.
#[derive(Debug, Default)]
pub struct SonioxParser {
    finals: Vec<Token>,
}

impl SonioxParser {
    fn flush(&mut self) -> Option<Parsed> {
        let tokens = std::mem::take(&mut self.finals);
        let mut words: Vec<Word> = Vec::new();
        let mut conf = (0.0f32, 0u32);
        for t in tokens {
            if let Some(c) = t.confidence {
                conf = (conf.0 + c, conf.1 + 1);
            }
            let speaker = t
                .speaker
                .as_deref()
                .and_then(|s| s.parse::<u32>().ok())
                .map(|n| n.saturating_sub(1));
            // A piece starting with a space (or a new speaker) starts a new word.
            let joins = !t.text.starts_with(char::is_whitespace)
                && words.last().is_some_and(|w| w.speaker == speaker);
            match words.last_mut() {
                Some(w) if joins => {
                    w.text.push_str(&t.text);
                    w.end_ms = t.end_ms;
                }
                _ => {
                    let text = t.text.trim_start().to_owned();
                    if !text.is_empty() {
                        words.push(Word {
                            text,
                            start_ms: t.start_ms,
                            end_ms: t.end_ms,
                            speaker,
                        });
                    }
                }
            }
        }
        if words.is_empty() {
            return None;
        }
        #[allow(clippy::cast_precision_loss, reason = "a token count")]
        let confidence = (conf.1 > 0).then(|| conf.0 / conf.1 as f32);
        Some(Parsed::Final { words, confidence })
    }
}

impl MessageParser for SonioxParser {
    fn parse(&mut self, text: &str) -> SidekickResult<Vec<Parsed>> {
        let Ok(msg) = serde_json::from_str::<Msg>(text) else {
            return Ok(vec![]);
        };
        if let Some(code) = msg.error_code {
            let code = match code {
                401 | 403 => ErrorCode::KeyInvalid,
                429 => ErrorCode::RateLimited,
                _ => ErrorCode::SttDisconnected,
            };
            return Err(SidekickError::new(
                code,
                format!("{NAME} reported an error."),
            ));
        }
        let mut out = Vec::new();
        let mut pending = String::new();
        for t in msg.tokens {
            if t.text == END {
                out.extend(self.flush());
            } else if t.is_final {
                self.finals.push(t);
            } else {
                pending.push_str(&t.text);
            }
        }
        if msg.finished {
            out.extend(self.flush());
        }
        let shown: String = self
            .finals
            .iter()
            .map(|t| t.text.as_str())
            .collect::<String>()
            + &pending;
        if !shown.trim().is_empty() {
            out.push(Parsed::Partial(shown.trim().to_owned()));
        }
        Ok(out)
    }
}

impl Dialect for SonioxProvider {
    fn name(&self) -> &'static str {
        NAME
    }

    fn request(&self, _cfg: &SttConfig, _key: &SecretString) -> SidekickResult<Request> {
        self.ws_url
            .as_str()
            .into_client_request()
            .map_err(|_| SidekickError::internal("Invalid Soniox URL."))
    }

    fn hello(&self, cfg: &SttConfig, key: &SecretString) -> Option<String> {
        Some(config_message(cfg, key))
    }

    fn parser(&self, _cfg: &SttConfig) -> Box<dyn MessageParser> {
        Box::new(SonioxParser::default())
    }

    fn keepalive(&self) -> Option<Message> {
        Some(Message::Text(r#"{"type":"keepalive"}"#.into()))
    }

    fn finish(&self) -> Option<Message> {
        Some(Message::Text(String::new().into()))
    }
}

#[async_trait::async_trait]
impl SttProvider for SonioxProvider {
    fn id(&self) -> &'static str {
        "soniox"
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
                "Add your Soniox API key in Settings.",
            )
        })?;
        super::ws::run(self, config, &key, frames, sink, cancel).await
    }
}

/// Checks a key with `GET /v1/models`.
#[derive(Debug, Clone)]
pub struct SonioxVerifier {
    api_url: String,
    http: reqwest::Client,
}

impl Default for SonioxVerifier {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL)
    }
}

impl SonioxVerifier {
    #[must_use]
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl KeyVerifier for SonioxVerifier {
    async fn verify(&self, key: &SecretString) -> SidekickResult<()> {
        let resp = self
            .http
            .get(format!("{}/v1/models", self.api_url))
            .bearer_auth(key.expose_secret())
            .send()
            .await
            .map_err(|_| {
                SidekickError::new(ErrorCode::ProviderUnavailable, "Couldn't reach Soniox.")
            })?;
        super::rest_status(NAME, resp.status().as_u16())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn config_carries_key_hints_diarization_and_terms() {
        let mut cfg = SttConfig::new(StreamKind::System);
        cfg.language = "multi".into();
        cfg.keyterms = vec!["Nitesh".into()];
        let c: serde_json::Value =
            serde_json::from_str(&config_message(&cfg, &SecretString::from("k".to_owned())))
                .unwrap();
        assert_eq!(c["api_key"], "k");
        assert_eq!(c["language_hints"], json!(["en", "hi"]));
        assert_eq!(c["enable_speaker_diarization"], true);
        assert_eq!(c["context"]["terms"], json!(["Nitesh"]));
        let mic = SttConfig::new(StreamKind::Mic);
        let c: serde_json::Value =
            serde_json::from_str(&config_message(&mic, &SecretString::from("k".to_owned())))
                .unwrap();
        assert_eq!(c["enable_speaker_diarization"], false);
        assert_eq!(c["language_hints"], json!(["en"]));
    }

    #[test]
    fn parses_a_recorded_session_into_words_and_speakers() {
        let fixture = include_str!("../../tests/fixtures/soniox/session.jsonl");
        let mut p = SonioxParser::default();
        let out: Vec<Parsed> = fixture
            .lines()
            .filter(|l| !l.is_empty())
            .flat_map(|l| p.parse(l).unwrap())
            .collect();
        assert_eq!(out[0], Parsed::Partial("Priya, what".into()));
        let finals: Vec<&Parsed> = out
            .iter()
            .filter(|p| matches!(p, Parsed::Final { .. }))
            .collect();
        assert_eq!(finals.len(), 1);
        let Parsed::Final { words, confidence } = finals[0] else {
            unreachable!()
        };
        let texts: Vec<_> = words.iter().map(|w| (w.text.as_str(), w.speaker)).collect();
        assert_eq!(
            texts,
            [
                ("Priya,", Some(0)),
                ("what's", Some(0)),
                ("the", Some(0)),
                ("limit?", Some(0)),
                ("Six", Some(1)),
                ("hundred.", Some(1))
            ]
        );
        assert_eq!((words[0].start_ms, words[3].end_ms), (1280, 3020));
        assert!(confidence.unwrap() > 0.9);
    }

    #[test]
    fn maps_errors() {
        let mut p = SonioxParser::default();
        let e = p
            .parse(r#"{"error_code":401,"error_message":"Invalid API key."}"#)
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::KeyInvalid);
        let e = p
            .parse(r#"{"error_code":503,"error_message":"busy"}"#)
            .unwrap_err();
        assert_eq!(e.code, ErrorCode::SttDisconnected);
    }

    #[test]
    fn finished_flushes_the_last_words() {
        let mut p = SonioxParser::default();
        p.parse(r#"{"tokens":[{"text":"Bye","start_ms":0,"end_ms":200,"is_final":true}]}"#)
            .unwrap();
        let out = p.parse(r#"{"tokens":[],"finished":true}"#).unwrap();
        assert!(matches!(&out[0], Parsed::Final { words, .. } if words[0].text == "Bye"));
    }
}
