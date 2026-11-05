//! Deepgram streaming adapter (ADR-005): Nova-3 over WebSocket.
//!
//! - Auth: `Authorization: Token <key>` on the WebSocket handshake.
//! - Audio: 16 kHz mono linear16, sent as binary messages; silence skipped.
//! - Text: interim results become partials; final results become segments.
//! - Keep-alive: `{"type":"KeepAlive"}` after `keepalive` of silence.
//! - Reconnect: frames are buffered (up to 10 s) while reconnecting and replayed.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use sidekick_audio::{AudioFrame, FRAME_MS, StreamKind};
use sidekick_core::keys::KeyVerifier;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::{ExposeSecret, SecretString};
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{self, Message, client::IntoClientRequest};
use tokio_util::sync::CancellationToken;

use crate::speakers::{self, Word};
use crate::timeline::Timeline;
use crate::{Partial, SttConfig, SttProvider, SttSink, SttStatus};

pub const DEFAULT_WS_URL: &str = "wss://api.deepgram.com/v1/listen";
pub const DEFAULT_API_URL: &str = "https://api.deepgram.com";
pub const MODEL: &str = "nova-3";
/// Frames kept while reconnecting (10 s, tech doc 5.4).
const MAX_BUFFER_FRAMES: usize = 200;
const BACKOFF: [Duration; 4] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

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

/// What a Deepgram message means for us.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    Partial(String),
    /// Final words in provider milliseconds.
    Final {
        words: Vec<Word>,
        confidence: Option<f32>,
    },
    Ignore,
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn ms(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1000.0).round() as u64
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

// ---- Session ----------------------------------------------------------------

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(url: &str, key: &SecretString) -> SidekickResult<Ws> {
    let mut req = url
        .into_client_request()
        .map_err(|_| SidekickError::internal("Invalid Deepgram URL."))?;
    let auth = format!("Token {}", key.expose_secret())
        .parse()
        .map_err(|_| {
            SidekickError::new(
                ErrorCode::KeyInvalid,
                "The Deepgram key has invalid characters.",
            )
        })?;
    req.headers_mut().insert("Authorization", auth);
    match tokio_tungstenite::connect_async(req).await {
        Ok((ws, _)) => Ok(ws),
        Err(tungstenite::Error::Http(resp)) => {
            let status = resp.status().as_u16();
            Err(match status {
                401 | 403 => {
                    SidekickError::new(ErrorCode::KeyInvalid, "Deepgram rejected the API key.")
                }
                429 => SidekickError::new(
                    ErrorCode::RateLimited,
                    "Deepgram is rate limiting. Retrying.",
                ),
                _ => SidekickError::new(
                    ErrorCode::SttDisconnected,
                    format!("Deepgram refused the connection (HTTP {status})."),
                ),
            })
        }
        Err(e) => {
            tracing::warn!(target: "stt", error = %e, "deepgram connect failed");
            Err(SidekickError::new(
                ErrorCode::SttDisconnected,
                "Couldn't reach Deepgram.",
            ))
        }
    }
}

fn frame_bytes(frame: &AudioFrame) -> Vec<u8> {
    frame.samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

enum Ended {
    /// The audio channel closed or we were cancelled: we're done.
    Finished,
    /// The connection dropped; reconnect.
    Dropped,
}

struct Session<'a> {
    cfg: &'a SttConfig,
    sink: &'a Arc<dyn SttSink>,
    timeline: Timeline,
    /// Frames since the last speech frame (for the tail).
    since_speech: u32,
    last_send: Instant,
    /// Sent frames not yet covered by a final result, with their provider end
    /// time. A dropped connection may have lost them, so they're replayed.
    unfinalized: VecDeque<(u64, AudioFrame)>,
}

impl Session<'_> {
    fn should_send(&mut self, frame: &AudioFrame) -> bool {
        if frame.speech {
            self.since_speech = 0;
            true
        } else {
            self.since_speech = self.since_speech.saturating_add(1);
            self.since_speech <= self.cfg.tail_frames
        }
    }

    async fn send(&mut self, ws: &mut Ws, frame: &AudioFrame) -> Result<(), tungstenite::Error> {
        ws.send(Message::Binary(frame_bytes(frame).into())).await?;
        self.timeline.sent(frame.start_ms, u64::from(FRAME_MS));
        self.last_send = Instant::now();
        if self.unfinalized.len() == MAX_BUFFER_FRAMES {
            self.unfinalized.pop_front();
        }
        self.unfinalized
            .push_back((self.timeline.sent_ms(), frame.clone()));
        Ok(())
    }

    fn handle_text(&mut self, text: &str) {
        match parse(text) {
            Parsed::Partial(t) => self.sink.partial(Partial {
                kind: self.cfg.kind,
                text: t,
            }),
            Parsed::Final { words, confidence } => {
                // Audio up to the end of this result is transcribed; stop keeping it.
                if let Some(end) = words.last().map(|w| w.end_ms) {
                    while self.unfinalized.front().is_some_and(|(e, _)| *e <= end) {
                        self.unfinalized.pop_front();
                    }
                }
                for mut seg in speakers::split(self.cfg.kind, &words, confidence) {
                    seg.start_ms = self.timeline.to_meeting(seg.start_ms);
                    seg.end_ms = self.timeline.to_meeting(seg.end_ms);
                    self.sink.segment(seg);
                }
            }
            Parsed::Ignore => {}
        }
    }

    /// Streams until the connection drops or the input ends.
    async fn stream(
        &mut self,
        ws: &mut Ws,
        frames: &mut Receiver<AudioFrame>,
        buffered: &mut VecDeque<AudioFrame>,
        cancel: &CancellationToken,
    ) -> Ended {
        // Replay audio captured while disconnected.
        while let Some(f) = buffered.pop_front() {
            if self.should_send(&f) && self.send(ws, &f).await.is_err() {
                buffered.push_front(f);
                return Ended::Dropped;
            }
        }
        let mut tick = tokio::time::interval(Duration::from_millis(250));
        loop {
            tokio::select! {
                () = cancel.cancelled() => {
                    let _ = ws.close(None).await;
                    return Ended::Finished;
                }
                frame = frames.recv() => match frame {
                    Some(f) => {
                        if self.should_send(&f) && self.send(ws, &f).await.is_err() {
                            buffered.push_back(f);
                            return Ended::Dropped;
                        }
                    }
                    None => {
                        // Ask Deepgram to flush, then read the last results.
                        let _ = ws.send(Message::Text(r#"{"type":"CloseStream"}"#.into())).await;
                        let drain = async {
                            while let Some(Ok(msg)) = ws.next().await {
                                if let Message::Text(t) = msg { self.handle_text(&t); }
                            }
                        };
                        let _ = tokio::time::timeout(Duration::from_secs(3), drain).await;
                        return Ended::Finished;
                    }
                },
                msg = ws.next() => match msg {
                    Some(Ok(Message::Text(t))) => self.handle_text(&t),
                    Some(Ok(Message::Close(_)) | Err(_)) | None => return Ended::Dropped,
                    Some(Ok(_)) => {}
                },
                _ = tick.tick() => {
                    if self.last_send.elapsed() >= self.cfg.keepalive {
                        if ws.send(Message::Text(r#"{"type":"KeepAlive"}"#.into())).await.is_err() {
                            return Ended::Dropped;
                        }
                        self.last_send = Instant::now();
                    }
                }
            }
        }
    }
}

fn buffer_push(buffer: &mut VecDeque<AudioFrame>, frame: AudioFrame) {
    if buffer.len() == MAX_BUFFER_FRAMES {
        buffer.pop_front();
    }
    buffer.push_back(frame);
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
        mut frames: Receiver<AudioFrame>,
        sink: Arc<dyn SttSink>,
        cancel: CancellationToken,
    ) -> SidekickResult<()> {
        let key = key.ok_or_else(|| {
            SidekickError::new(
                ErrorCode::KeyMissing,
                "Add your Deepgram API key in Settings.",
            )
        })?;
        let url = self.listen_url(&config);
        let kind = config.kind;
        let mut buffered: VecDeque<AudioFrame> = VecDeque::new();
        let mut first = true;
        loop {
            sink.status(
                kind,
                if first {
                    SttStatus::Connecting
                } else {
                    SttStatus::Reconnecting
                },
            );
            let started = Instant::now();
            let mut attempt = 0usize;
            let mut ws = loop {
                let connecting = connect(&url, &key);
                tokio::pin!(connecting);
                // Keep buffering audio while connecting.
                let result = loop {
                    tokio::select! {
                        () = cancel.cancelled() => return Ok(()),
                        r = &mut connecting => break r,
                        f = frames.recv() => match f {
                            Some(f) => buffer_push(&mut buffered, f),
                            None if buffered.is_empty() => return Ok(()),
                            None => {}
                        },
                    }
                };
                match result {
                    Ok(ws) => break ws,
                    Err(e) if e.code == ErrorCode::KeyInvalid => {
                        sink.status(kind, SttStatus::Failed);
                        return Err(e);
                    }
                    Err(e) => {
                        if started.elapsed() >= config.reconnect_deadline {
                            sink.status(kind, SttStatus::Failed);
                            return Err(e);
                        }
                        let wait = BACKOFF[attempt.min(BACKOFF.len() - 1)];
                        attempt += 1;
                        tokio::select! {
                            () = cancel.cancelled() => return Ok(()),
                            () = tokio::time::sleep(wait) => {}
                        }
                    }
                }
            };
            sink.status(kind, SttStatus::Live);
            first = false;
            let mut session = Session {
                cfg: &config,
                sink: &sink,
                timeline: Timeline::default(),
                since_speech: u32::MAX,
                last_send: Instant::now(),
                unfinalized: VecDeque::new(),
            };
            match session
                .stream(&mut ws, &mut frames, &mut buffered, &cancel)
                .await
            {
                Ended::Finished => return Ok(()),
                Ended::Dropped => {
                    tracing::warn!(target: "stt", ?kind, replay = session.unfinalized.len(), "deepgram connection dropped; reconnecting");
                    // Replay audio the provider may not have transcribed, before newer buffered audio.
                    for (_, f) in session.unfinalized.drain(..).rev() {
                        buffered.push_front(f);
                    }
                    while buffered.len() > MAX_BUFFER_FRAMES {
                        buffered.pop_front();
                    }
                }
            }
        }
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
