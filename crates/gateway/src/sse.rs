//! Incremental Server-Sent Events parser. Bytes arrive in arbitrary chunks;
//! complete events come out. Handles `\n` and `\r\n`, multi-line `data:`, and
//! comment lines (`:` keep-alives).

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    pub event: Option<String>,
    pub data: String,
}

#[derive(Debug, Default)]
pub struct SseParser {
    buffer: String,
    event: Option<String>,
    data: Vec<String>,
}

impl SseParser {
    /// Feeds a chunk and returns every event it completes.
    pub fn push(&mut self, chunk: &str) -> Vec<SseEvent> {
        self.buffer.push_str(chunk);
        let mut out = Vec::new();
        while let Some(pos) = self.buffer.find('\n') {
            let mut line: String = self.buffer.drain(..=pos).collect();
            line.pop(); // '\n'
            if line.ends_with('\r') {
                line.pop();
            }
            if line.is_empty() {
                if !self.data.is_empty() || self.event.is_some() {
                    out.push(SseEvent {
                        event: self.event.take(),
                        data: self.data.join("\n"),
                    });
                    self.data.clear();
                }
            } else if line.starts_with(':') {
                // Comment / keep-alive.
            } else {
                let (field, value) = line.split_once(':').unwrap_or((line.as_str(), ""));
                let value = value.strip_prefix(' ').unwrap_or(value);
                match field {
                    "event" => self.event = Some(value.to_owned()),
                    "data" => self.data.push(value.to_owned()),
                    _ => {}
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_split_across_chunks() {
        let mut p = SseParser::default();
        assert!(p.push("event: ping\nda").is_empty());
        let events = p.push("ta: {\"a\":1}\n\nevent: x\r\ndata: 2\r\n\r\n");
        assert_eq!(
            events,
            vec![
                SseEvent {
                    event: Some("ping".into()),
                    data: "{\"a\":1}".into()
                },
                SseEvent {
                    event: Some("x".into()),
                    data: "2".into()
                },
            ]
        );
    }

    #[test]
    fn joins_multi_line_data_and_skips_comments() {
        let mut p = SseParser::default();
        let events = p.push(": keep-alive\ndata: a\ndata: b\n\n");
        assert_eq!(
            events,
            vec![SseEvent {
                event: None,
                data: "a\nb".into()
            }]
        );
    }
}
