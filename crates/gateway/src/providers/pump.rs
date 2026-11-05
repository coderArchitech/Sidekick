//! Turns a streaming HTTP response into a `TokenStream`, for every adapter.
//!
//! Bytes are decoded as UTF-8 across chunk boundaries (a character split
//! between two network reads is kept until it's complete), then framed as
//! Server-Sent Events (Anthropic, OpenAI, Gemini) or newline-delimited JSON
//! (Ollama), and handed to the adapter's [`Handler`].

use futures::StreamExt;
use sidekick_core::llm::{StreamEvent, TokenStream};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use super::http::transport_error;
use crate::sse::SseParser;

/// How the response body is framed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Framing {
    Sse,
    /// One JSON object per line.
    Lines,
}

/// Provider-specific parsing of one framed message.
pub trait Handler: Send + 'static {
    /// `event` is the SSE event name (`None` for NDJSON). Return an event to
    /// emit; `Done` or an error ends the stream.
    fn on_message(
        &mut self,
        event: Option<&str>,
        data: &str,
    ) -> SidekickResult<Option<StreamEvent>>;

    /// The body ended without a `Done`. Providers whose stream simply ends
    /// (Gemini) finish here; the default treats it as a dropped connection.
    fn on_end(&mut self) -> SidekickResult<StreamEvent> {
        Err(SidekickError::new(
            ErrorCode::ProviderUnavailable,
            "The answer stream ended early.",
        ))
    }
}

/// Decodes UTF-8 incrementally.
#[derive(Debug, Default)]
pub struct Utf8Decoder {
    pending: Vec<u8>,
}

impl Utf8Decoder {
    /// Returns the text that is complete so far; keeps a split character.
    pub fn push(&mut self, bytes: &[u8]) -> String {
        self.pending.extend_from_slice(bytes);
        let valid = match std::str::from_utf8(&self.pending) {
            Ok(_) => self.pending.len(),
            // An incomplete sequence at the end waits for more bytes;
            // anything else invalid is replaced so the stream keeps going.
            Err(e) if e.error_len().is_none() => e.valid_up_to(),
            Err(_) => {
                let text = String::from_utf8_lossy(&self.pending).into_owned();
                self.pending.clear();
                return text;
            }
        };
        let rest = self.pending.split_off(valid);
        String::from_utf8(std::mem::replace(&mut self.pending, rest)).unwrap_or_default()
    }
}

/// Splits NDJSON into lines across chunks.
#[derive(Debug, Default)]
struct LineSplitter {
    buffer: String,
}

impl LineSplitter {
    fn push(&mut self, text: &str) -> Vec<String> {
        self.buffer.push_str(text);
        let mut out = Vec::new();
        while let Some(pos) = self.buffer.find('\n') {
            let line: String = self.buffer.drain(..=pos).collect();
            let line = line.trim();
            if !line.is_empty() {
                out.push(line.to_owned());
            }
        }
        out
    }

    fn finish(&mut self) -> Option<String> {
        let rest = std::mem::take(&mut self.buffer);
        let rest = rest.trim();
        (!rest.is_empty()).then(|| rest.to_owned())
    }
}

/// Streams `resp` through `handler`.
pub fn pump(
    resp: reqwest::Response,
    provider: &'static str,
    framing: Framing,
    mut handler: impl Handler,
) -> TokenStream {
    let mut bytes = resp.bytes_stream();
    super::anthropic::async_stream(move |tx| async move {
        let mut utf8 = Utf8Decoder::default();
        let mut sse = SseParser::default();
        let mut lines = LineSplitter::default();
        // Sends one handler result; returns false when the stream is over.
        let emit = |r: SidekickResult<Option<StreamEvent>>| {
            let tx = tx.clone();
            async move {
                match r {
                    Ok(Some(ev)) => {
                        let done = matches!(ev, StreamEvent::Done { .. });
                        tx.send(Ok(ev)).await.is_ok() && !done
                    }
                    Ok(None) => true,
                    Err(e) => {
                        let _ = tx.send(Err(e)).await;
                        false
                    }
                }
            }
        };
        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(transport_error(provider, &e))).await;
                    return;
                }
            };
            let text = utf8.push(&chunk);
            let messages: Vec<(Option<String>, String)> = match framing {
                Framing::Sse => sse
                    .push(&text)
                    .into_iter()
                    .map(|e| (e.event, e.data))
                    .collect(),
                Framing::Lines => lines.push(&text).into_iter().map(|l| (None, l)).collect(),
            };
            for (event, data) in messages {
                if !emit(handler.on_message(event.as_deref(), &data)).await {
                    return;
                }
            }
        }
        if framing == Framing::Lines
            && let Some(last) = lines.finish()
            && !emit(handler.on_message(None, &last)).await
        {
            return;
        }
        let _ = emit(handler.on_end().map(Some)).await;
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_split_across_chunks_is_kept_whole() {
        let bytes = "a — b".as_bytes(); // the dash is 3 bytes
        let mut d = Utf8Decoder::default();
        let mut out = d.push(&bytes[..2]);
        assert_eq!(out, "a ");
        out.push_str(&d.push(&bytes[2..3]));
        assert_eq!(out, "a ", "half a character waits");
        out.push_str(&d.push(&bytes[3..]));
        assert_eq!(out, "a — b");
    }

    #[test]
    fn invalid_bytes_are_replaced_not_fatal() {
        let mut d = Utf8Decoder::default();
        assert_eq!(d.push(&[b'x', 0xFF, b'y']), "x\u{FFFD}y");
    }

    #[test]
    fn lines_split_across_chunks() {
        let mut l = LineSplitter::default();
        assert!(l.push("{\"a\":").is_empty());
        assert_eq!(
            l.push("1}\n\n{\"b\":2}\n{\"c\""),
            vec!["{\"a\":1}", "{\"b\":2}"]
        );
        assert_eq!(l.finish().as_deref(), Some("{\"c\""));
    }
}
