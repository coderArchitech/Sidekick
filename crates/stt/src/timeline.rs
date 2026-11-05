//! Maps provider audio time back to meeting time.
//!
//! When silence isn't sent, the provider's clock only advances for audio it
//! received. Each time sending resumes after a gap, we record a breakpoint
//! (provider ms, meeting ms); any provider time maps from the last breakpoint
//! at or before it.

#[derive(Debug, Clone, Default)]
pub struct Timeline {
    /// (provider_ms, meeting_ms), increasing in both.
    breakpoints: Vec<(u64, u64)>,
    sent_ms: u64,
    last_meeting_end: Option<u64>,
}

impl Timeline {
    /// Records that a frame starting at `meeting_ms`, `len_ms` long, was sent.
    pub fn sent(&mut self, meeting_ms: u64, len_ms: u64) {
        let contiguous = self.last_meeting_end == Some(meeting_ms);
        if !contiguous {
            self.breakpoints.push((self.sent_ms, meeting_ms));
        }
        self.sent_ms += len_ms;
        self.last_meeting_end = Some(meeting_ms + len_ms);
    }

    /// Total provider milliseconds sent on this connection.
    #[must_use]
    pub fn sent_ms(&self) -> u64 {
        self.sent_ms
    }

    /// Meeting time for a provider timestamp.
    #[must_use]
    pub fn to_meeting(&self, provider_ms: u64) -> u64 {
        let idx = self.breakpoints.partition_point(|(p, _)| *p <= provider_ms);
        match idx.checked_sub(1).and_then(|i| self.breakpoints.get(i)) {
            Some((p, m)) => m + (provider_ms - p),
            None => provider_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contiguous_audio_maps_with_the_start_offset() {
        let mut t = Timeline::default();
        for i in 0..10 {
            t.sent(5_000 + i * 50, 50);
        }
        assert_eq!(t.to_meeting(0), 5_000);
        assert_eq!(t.to_meeting(320), 5_320);
    }

    #[test]
    fn skipped_silence_is_added_back() {
        let mut t = Timeline::default();
        // 0–1000 ms sent, 1000–4000 ms silence skipped, 4000–5000 ms sent.
        for i in 0..20 {
            t.sent(i * 50, 50);
        }
        for i in 0..20 {
            t.sent(4_000 + i * 50, 50);
        }
        assert_eq!(t.to_meeting(500), 500);
        assert_eq!(
            t.to_meeting(1_000),
            4_000,
            "provider 1000 ms is meeting 4000 ms"
        );
        assert_eq!(t.to_meeting(1_500), 4_500);
    }

    #[test]
    fn a_new_connection_starts_a_fresh_timeline() {
        let mut t = Timeline::default();
        t.sent(60_000, 50);
        assert_eq!(t.to_meeting(10), 60_010);
    }
}
