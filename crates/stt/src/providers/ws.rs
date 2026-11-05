//! The streaming engine every WebSocket STT provider shares: connect with
//! backoff, buffer audio while (re)connecting (up to 10 s), skip silence apart
//! from a short tail, keep-alives, replay audio a dropped connection may have
//! lost, and flush at the end. A [`Dialect`] supplies what differs per
//! provider: URL and auth, the first message, keep-alive and finish messages,
//! and how to read results.

use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::{SinkExt, StreamExt};
use sidekick_audio::{AudioFrame, FRAME_MS};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_secrets::SecretString;
use tokio::sync::mpsc::Receiver;
use tokio_tungstenite::tungstenite::{self, Message, handshake::client::Request};
use tokio_util::sync::CancellationToken;

use crate::speakers::{self, Word};
use crate::timeline::Timeline;
use crate::{Partial, SttConfig, SttSink, SttStatus};

/// Frames kept while reconnecting (10 s, tech doc 5.4).
pub const MAX_BUFFER_FRAMES: usize = 200;
const BACKOFF: [Duration; 4] = [
    Duration::from_millis(250),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
];

/// What one provider message means for us.
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

/// Reads one provider's messages. May keep state between messages (Soniox
/// sends final tokens one by one).
pub trait MessageParser: Send {
    /// An error ends the connection: `KEY_INVALID` stops the run, anything
    /// else reconnects.
    fn parse(&mut self, text: &str) -> SidekickResult<Vec<Parsed>>;
}

pub trait Dialect: Send + Sync {
    /// Display name for messages ("Deepgram").
    fn name(&self) -> &'static str;
    /// The handshake request, with auth.
    fn request(&self, cfg: &SttConfig, key: &SecretString) -> SidekickResult<Request>;
    /// A text message sent right after connecting (Soniox's config).
    fn hello(&self, _cfg: &SttConfig, _key: &SecretString) -> Option<String> {
        None
    }
    fn parser(&self, cfg: &SttConfig) -> Box<dyn MessageParser>;
    /// Sent after `keepalive` of silence, if the provider has one.
    fn keepalive(&self) -> Option<Message>;
    /// Sent when the audio ends, so the provider flushes its last results.
    fn finish(&self) -> Option<Message>;
}

pub(crate) fn auth_header(
    value: &str,
    provider: &str,
) -> SidekickResult<tungstenite::http::HeaderValue> {
    value.parse().map_err(|_| {
        SidekickError::new(
            ErrorCode::KeyInvalid,
            format!("The {provider} key has invalid characters."),
        )
    })
}

#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn ms(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1000.0).round() as u64
}

type Ws =
    tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;

async fn connect(dialect: &dyn Dialect, cfg: &SttConfig, key: &SecretString) -> SidekickResult<Ws> {
    let name = dialect.name();
    let req = dialect.request(cfg, key)?;
    let mut ws = match tokio_tungstenite::connect_async(req).await {
        Ok((ws, _)) => ws,
        Err(tungstenite::Error::Http(resp)) => {
            let status = resp.status().as_u16();
            return Err(match status {
                401 | 403 => SidekickError::new(
                    ErrorCode::KeyInvalid,
                    format!("{name} rejected the API key."),
                ),
                429 => SidekickError::new(
                    ErrorCode::RateLimited,
                    format!("{name} is rate limiting. Retrying."),
                ),
                _ => SidekickError::new(
                    ErrorCode::SttDisconnected,
                    format!("{name} refused the connection (HTTP {status})."),
                ),
            });
        }
        Err(e) => {
            tracing::warn!(target: "stt", provider = name, error = %e, "connect failed");
            return Err(SidekickError::new(
                ErrorCode::SttDisconnected,
                format!("Couldn't reach {name}."),
            ));
        }
    };
    if let Some(hello) = dialect.hello(cfg, key) {
        ws.send(Message::Text(hello.into())).await.map_err(|_| {
            SidekickError::new(
                ErrorCode::SttDisconnected,
                format!("Couldn't reach {name}."),
            )
        })?;
    }
    Ok(ws)
}

fn frame_bytes(frame: &AudioFrame) -> Vec<u8> {
    frame.samples.iter().flat_map(|s| s.to_le_bytes()).collect()
}

enum Ended {
    /// The audio channel closed or we were cancelled: we're done.
    Finished,
    /// The connection dropped; reconnect.
    Dropped,
    /// The provider reported an error that retrying won't fix.
    Failed(SidekickError),
}

struct Session<'a> {
    dialect: &'a dyn Dialect,
    cfg: &'a SttConfig,
    sink: &'a Arc<dyn SttSink>,
    parser: Box<dyn MessageParser>,
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

    /// Returns an error the run can't recover from by reconnecting.
    fn handle_text(&mut self, text: &str) -> Result<(), SidekickError> {
        let parsed = match self.parser.parse(text) {
            Ok(p) => p,
            Err(e) if e.code == ErrorCode::KeyInvalid => return Err(e),
            Err(e) => {
                tracing::warn!(target: "stt", provider = self.dialect.name(), code = ?e.code, "provider error");
                return Err(e);
            }
        };
        for p in parsed {
            match p {
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
        Ok(())
    }

    fn on_text(&mut self, text: &str) -> Option<Ended> {
        match self.handle_text(text) {
            Ok(()) => None,
            Err(e) if e.code == ErrorCode::KeyInvalid => Some(Ended::Failed(e)),
            Err(_) => Some(Ended::Dropped),
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
                        // Ask the provider to flush, then read the last results.
                        if let Some(m) = self.dialect.finish() {
                            let _ = ws.send(m).await;
                        }
                        let drain = async {
                            while let Some(Ok(msg)) = ws.next().await {
                                if let Message::Text(t) = msg
                                    && self.handle_text(&t).is_err()
                                {
                                    break;
                                }
                            }
                        };
                        let _ = tokio::time::timeout(Duration::from_secs(3), drain).await;
                        return Ended::Finished;
                    }
                },
                msg = ws.next() => match msg {
                    Some(Ok(Message::Text(t))) => {
                        if let Some(end) = self.on_text(&t) {
                            return end;
                        }
                    }
                    Some(Ok(Message::Close(_)) | Err(_)) | None => return Ended::Dropped,
                    Some(Ok(_)) => {}
                },
                _ = tick.tick() => {
                    if self.last_send.elapsed() >= self.cfg.keepalive {
                        if let Some(m) = self.dialect.keepalive()
                            && ws.send(m).await.is_err()
                        {
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

/// Runs one stream through `dialect` until the audio ends or `cancel` fires.
pub async fn run(
    dialect: &dyn Dialect,
    config: SttConfig,
    key: &SecretString,
    mut frames: Receiver<AudioFrame>,
    sink: Arc<dyn SttSink>,
    cancel: CancellationToken,
) -> SidekickResult<()> {
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
            let connecting = connect(dialect, &config, key);
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
            dialect,
            cfg: &config,
            sink: &sink,
            parser: dialect.parser(&config),
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
            Ended::Failed(e) => {
                sink.status(kind, SttStatus::Failed);
                return Err(e);
            }
            Ended::Dropped => {
                tracing::warn!(target: "stt", provider = dialect.name(), ?kind, replay = session.unfinalized.len(), "connection dropped; reconnecting");
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
