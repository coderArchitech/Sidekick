//! Deepgram streaming adapter (ADR-005): Nova-3 over WebSocket.
//!
//! - Auth: `Authorization: Token <key>` on the WebSocket handshake.
//! - Audio: 16 kHz mono linear16, sent as binary messages; silence skipped.
//! - Text: interim results become partials; final results become segments.
//! - Keep-alive: `{"type":"KeepAlive"}` after `keepalive` of silence.
//! - Reconnect: frames are buffered (up to 10 s) while reconnecting and replayed.

use std::sync::Arc;

use serde::Deserialize;
use sidekick_audio::{AudioFrame, StreamKind};
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{
    Message, client::IntoClientRequest, handshake::client::Request,
};
use tokio_util::sync::CancellationToken;

pub use super::ws::Parsed;
use super::ws::{Dialect, MessageParser, auth_header, ms};
use crate::speakers::Word;
use crate::{SttConfig, SttProvider, SttSink};

pub const DEFAULT_WS_URL: &str = "wss://api.deepgram.com/v1/listen";
pub const DEFAULT_API_URL: &str = "https://api.deepgram.com";
pub const MODEL: &str = "nova-3";

#[derive(Debug, Clone)]
pub struct DeepgramProvider {
    ws_url: String,
}

impl Default for DeepgramProvider {
    fn default() -> Self {
        Self::new(DEFAULT_WS_URL)
    }
}

impl DeepgramProvider {
    /// `ws_url` is overridable so tests can use a local server.
    #[must_use]
    pub fn new(ws_url: impl Into<String>) -> Self {
        Self {
            ws_url: ws_url.into(),
        }
    }

    /// The listen URL with query parameters for this run.
    #[must_use]
    pub fn listen_url(&self, cfg: &SttConfig) -> String {
        let mut url = url::Url::parse(&self.ws_url).unwrap_or_else(|_| {
            #[allow(clippy::expect_used)] // DEFAULT_WS_URL is a valid constant.
            url::Url::parse(DEFAULT_WS_URL).expect("valid default URL")
        });
        {
            let mut q = url.query_pairs_mut();
            q.append_pair("model", MODEL)
                .append_pair("encoding", "linear16")
                .append_pair("sample_rate", "16000")
                .append_pair("channels", "1")
                .append_pair("interim_results", "true")
                .append_pair("smart_format", "true")
                .append_pair("endpointing", "300")
                .append_pair("language", &cfg.language);
            if cfg.kind == StreamKind::System {
                q.append_pair("diarize", "true");
            }
            for term in &cfg.keyterms {
                q.append_pair("keyterm", term);
            }
        }
        url.into()
    }
}

// ---- Messages ---------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ResultsMsg {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    is_final: bool,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    duration: f64,
    channel: Option<Channel>,
}

#[derive(Debug, Deserialize)]
struct Channel {
    alternatives: Vec<Alternative>,
}

#[derive(Debug, Deserialize)]
struct Alternative {
    transcript: String,
    confidence: Option<f32>,
    #[serde(default)]
    words: Vec<DgWord>,
}

#[derive(Debug, Deserialize)]
struct DgWord {
    word: String,
    punctuated_word: Option<String>,
    start: f64,
    end: f64,
    speaker: Option<u32>,
}

/// Parses one text message from Deepgram.
#[must_use]
pub fn parse(text: &str) -> Parsed {
    let Ok(msg) = serde_json::from_str::<ResultsMsg>(text) else {
        return Parsed::Ignore;
    };
    if msg.kind != "Results" {
        return Parsed::Ignore; // Metadata, SpeechStarted, UtteranceEnd
    }
    let Some(alt) = msg.channel.and_then(|c| c.alternatives.into_iter().next()) else {
        return Parsed::Ignore;
    };
    if alt.transcript.trim().is_empty() {
        return Parsed::Ignore;
    }
    if !msg.is_final {
        return Parsed::Partial(alt.transcript);
    }
    let words = if alt.words.is_empty() {
        vec![Word {
            text: alt.transcript,
            start_ms: ms(msg.start),
            end_ms: ms(msg.start + msg.duration),
            speaker: None,
        }]
    } else {
        alt.words
            .into_iter()
            .map(|w| Word {
                text: w.punctuated_word.unwrap_or(w.word),
                start_ms: ms(w.start),
                end_ms: ms(w.end),
                speaker: w.speaker,
            })
            .collect()
    };
    Parsed::Final {
        words,
        confidence: alt.confidence,
    }
}

// ---- Dialect --------------------------------------------------------------

struct DeepgramParser;

impl MessageParser for DeepgramParser {
    fn parse(&mut self, text: &str) -> SidekickResult<Vec<Parsed>> {
        Ok(vec![parse(text)])
    }
}

impl Dialect for DeepgramProvider {
    fn name(&self) -> &'static str {
        "Deepgram"
    }

    fn request(&self, cfg: &SttConfig, key: &SecretString) -> SidekickResult<Request> {
        let mut req = self
            .listen_url(cfg)
            .into_client_request()
            .map_err(|_| SidekickError::internal("Invalid Deepgram URL."))?;
        req.headers_mut().insert(
            "Authorization",
            auth_header(&format!("Token {}", key.expose_secret()), "Deepgram")?,
        );
        Ok(req)
    }

    fn parser(&self, _cfg: &SttConfig) -> Box<dyn MessageParser> {
        Box::new(DeepgramParser)
    }

    fn keepalive(&self) -> Option<Message> {
        Some(Message::Text(r#"{"type":"KeepAlive"}"#.into()))
    }

    fn finish(&self) -> Option<Message> {
        Some(Message::Text(r#"{"type":"CloseStream"}"#.into()))
    }
}

#[async_trait::async_trait]
impl SttProvider for DeepgramProvider {
    fn id(&self) -> &'static str {
        "deepgram"
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
                "Add your Deepgram API key in Settings.",
            )
        })?;
        super::ws::run(self, config, &key, frames, sink, cancel).await
    }
}

/// Checks a Deepgram key with a lightweight REST call (`GET /v1/projects`).
#[derive(Debug, Clone)]
pub struct DeepgramVerifier {
    api_url: String,
    http: reqwest::Client,
}

impl Default for DeepgramVerifier {
    fn default() -> Self {
        Self::new(DEFAULT_API_URL)
    }
}

impl DeepgramVerifier {
    #[must_use]
    pub fn new(api_url: impl Into<String>) -> Self {
        Self {
            api_url: api_url.into().trim_end_matches('/').to_owned(),
            http: reqwest::Client::new(),
        }
    }
}

#[async_trait::async_trait]
impl KeyVerifier for DeepgramVerifier {
    async fn verify(&self, key: &SecretString) -> SidekickResult<()> {
        let resp = self
            .http
            .get(format!("{}/v1/projects", self.api_url))
            .header("Authorization", format!("Token {}", key.expose_secret()))
            .send()
            .await
            .map_err(|_| {
                SidekickError::new(ErrorCode::ProviderUnavailable, "Couldn't reach Deepgram.")
            })?;
        match resp.status().as_u16() {
            200..=299 => Ok(()),
            401 | 403 => Err(SidekickError::new(
                ErrorCode::KeyInvalid,
                "Deepgram rejected the API key.",
            )),
            429 => Err(SidekickError::new(
                ErrorCode::RateLimited,
                "Deepgram is rate limiting requests.",
            )),
            s => Err(SidekickError::new(
                ErrorCode::ProviderUnavailable,
                format!("Deepgram is unavailable (HTTP {s})."),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_the_listen_url() {
        let mut cfg = SttConfig::new(StreamKind::System);
        cfg.keyterms = vec!["Nitesh".into(), "Sidekick".into()];
        let url = DeepgramProvider::default().listen_url(&cfg);
        assert!(url.starts_with(
            "wss://api.deepgram.com/v1/listen?model=nova-3&encoding=linear16&sample_rate=16000"
        ));
        assert!(url.contains("diarize=true"));
        assert!(url.contains("keyterm=Nitesh&keyterm=Sidekick"));
        assert!(url.contains("interim_results=true"));
        assert!(
            !DeepgramProvider::default()
                .listen_url(&SttConfig::new(StreamKind::Mic))
                .contains("diarize")
        );
    }

    #[test]
    fn parses_interim_and_final_results() {
        let interim = r#"{"type":"Results","is_final":false,"start":0.0,"duration":1.0,"channel":{"alternatives":[{"transcript":"what is the","confidence":0.8,"words":[]}]}}"#;
        assert_eq!(parse(interim), Parsed::Partial("what is the".into()));

        let fin = r#"{"type":"Results","is_final":true,"speech_final":true,"start":1.0,"duration":1.5,
            "channel":{"alternatives":[{"transcript":"priya what's the limit","confidence":0.97,"words":[
              {"word":"priya","punctuated_word":"Priya,","start":1.0,"end":1.4,"speaker":0},
              {"word":"limit","punctuated_word":"limit?","start":1.9,"end":2.5,"speaker":0}]}]}}"#;
        let Parsed::Final { words, confidence } = parse(fin) else {
            panic!("expected final")
        };
        assert_eq!(confidence, Some(0.97));
        assert_eq!(
            words[0],
            Word {
                text: "Priya,".into(),
                start_ms: 1_000,
                end_ms: 1_400,
                speaker: Some(0)
            }
        );
    }

    #[test]
    fn ignores_metadata_and_empty_results() {
        assert_eq!(
            parse(r#"{"type":"Metadata","request_id":"x"}"#),
            Parsed::Ignore
        );
        assert_eq!(
            parse(r#"{"type":"SpeechStarted","timestamp":1.0}"#),
            Parsed::Ignore
        );
        let empty = r#"{"type":"Results","is_final":true,"channel":{"alternatives":[{"transcript":"","words":[]}]}}"#;
        assert_eq!(parse(empty), Parsed::Ignore);
        assert_eq!(parse("not json"), Parsed::Ignore);
    }

    #[test]
    fn final_without_words_uses_result_times() {
        let fin = r#"{"type":"Results","is_final":true,"start":2.0,"duration":0.5,"channel":{"alternatives":[{"transcript":"okay","words":[]}]}}"#;
        let Parsed::Final { words, .. } = parse(fin) else {
            panic!()
        };
        assert_eq!((words[0].start_ms, words[0].end_ms), (2_000, 2_500));
    }
}
