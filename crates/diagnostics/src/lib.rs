//! Latency diagnostics (tech doc 10.1, 13.1; NFR-1, NFR-2).
//!
//! Each pipeline stage keeps its recent samples (last 15 minutes, at most 1,000)
//! and reports count, p50, p95 and whether it meets its target. Everything stays
//! on this machine; only durations are stored, never content.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const WINDOW: Duration = Duration::from_secs(15 * 60);
const MAX_SAMPLES: usize = 1_000;

/// Measured stages. Targets come from tech doc 10.1 and 13.1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    /// End of speech to final segment received.
    TranscriptLag,
    /// Question end to first cue token.
    QuestionToCue,
    /// Question end to first full-answer token.
    QuestionToAnswer,
    /// Request sent to first token, per LLM call.
    LlmFirstToken,
    /// Context retrieval for one question.
    Retrieval,
}

impl Stage {
    pub const ALL: [Self; 5] = [
        Self::TranscriptLag,
        Self::QuestionToCue,
        Self::QuestionToAnswer,
        Self::LlmFirstToken,
        Self::Retrieval,
    ];

    /// (p50 target, p95 target) in ms.
    #[must_use]
    pub const fn targets(self) -> (u32, u32) {
        match self {
            Self::TranscriptLag => (1_000, 1_500),
            Self::QuestionToCue => (2_000, 3_000),
            Self::QuestionToAnswer => (2_500, 4_000),
            Self::LlmFirstToken => (900, 2_000),
            Self::Retrieval => (100, 250),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct StageStats {
    pub stage: Stage,
    pub count: u32,
    pub p50_ms: Option<u32>,
    pub p95_ms: Option<u32>,
    pub target_p50_ms: u32,
    pub target_p95_ms: u32,
    /// `None` until there are samples.
    pub healthy: Option<bool>,
}

#[derive(Debug, Default)]
pub struct Recorder {
    samples: Mutex<HashMap<Stage, VecDeque<(Instant, u32)>>>,
}

impl Recorder {
    pub fn record(&self, stage: Stage, duration: Duration) {
        self.record_at(stage, duration, Instant::now());
    }

    fn record_at(&self, stage: Stage, duration: Duration, now: Instant) {
        let ms = u32::try_from(duration.as_millis()).unwrap_or(u32::MAX);
        let mut map = self.samples.lock().unwrap_or_else(PoisonError::into_inner);
        let q = map.entry(stage).or_default();
        q.push_back((now, ms));
        prune(q, now);
    }

    /// Stats for every stage, in `Stage::ALL` order.
    #[must_use]
    pub fn snapshot(&self) -> Vec<StageStats> {
        self.snapshot_at(Instant::now())
    }

    fn snapshot_at(&self, now: Instant) -> Vec<StageStats> {
        let mut map = self.samples.lock().unwrap_or_else(PoisonError::into_inner);
        Stage::ALL
            .iter()
            .map(|&stage| {
                let q = map.entry(stage).or_default();
                prune(q, now);
                let mut values: Vec<u32> = q.iter().map(|(_, ms)| *ms).collect();
                values.sort_unstable();
                let (p50, p95) = (percentile(&values, 50), percentile(&values, 95));
                let (t50, t95) = stage.targets();
                StageStats {
                    stage,
                    count: u32::try_from(values.len()).unwrap_or(u32::MAX),
                    p50_ms: p50,
                    p95_ms: p95,
                    target_p50_ms: t50,
                    target_p95_ms: t95,
                    healthy: p50.zip(p95).map(|(a, b)| a <= t50 && b <= t95),
                }
            })
            .collect()
    }
}

fn prune(q: &mut VecDeque<(Instant, u32)>, now: Instant) {
    while q
        .front()
        .is_some_and(|(t, _)| now.duration_since(*t) > WINDOW)
        || q.len() > MAX_SAMPLES
    {
        q.pop_front();
    }
}

/// Nearest-rank percentile of sorted values.
#[must_use]
pub fn percentile(sorted: &[u32], pct: u32) -> Option<u32> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (u64::from(pct) * sorted.len() as u64).div_ceil(100).max(1);
    let idx = usize::try_from(rank - 1).unwrap_or(0).min(sorted.len() - 1);
    Some(sorted[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_rank_percentiles() {
        let v: Vec<u32> = (1..=100).collect();
        assert_eq!(percentile(&v, 50), Some(50));
        assert_eq!(percentile(&v, 95), Some(95));
        assert_eq!(percentile(&[7], 95), Some(7));
        assert_eq!(percentile(&[], 50), None);
        assert_eq!(percentile(&[1, 2, 3, 4], 50), Some(2));
    }

    #[test]
    fn reports_health_against_targets() {
        let r = Recorder::default();
        for ms in [800, 900, 1_000, 1_100, 1_200] {
            r.record(Stage::QuestionToCue, Duration::from_millis(ms));
        }
        let cue = r
            .snapshot()
            .into_iter()
            .find(|s| s.stage == Stage::QuestionToCue)
            .unwrap();
        assert_eq!(
            (cue.count, cue.p50_ms, cue.p95_ms, cue.healthy),
            (5, Some(1_000), Some(1_200), Some(true))
        );

        r.record(Stage::Retrieval, Duration::from_millis(400));
        let retrieval = r
            .snapshot()
            .into_iter()
            .find(|s| s.stage == Stage::Retrieval)
            .unwrap();
        assert_eq!(retrieval.healthy, Some(false));
    }

    #[test]
    fn empty_stages_have_no_verdict() {
        let s = Recorder::default().snapshot();
        assert_eq!(s.len(), Stage::ALL.len());
        assert!(s.iter().all(|x| x.count == 0 && x.healthy.is_none()));
    }

    #[test]
    fn rolling_window_drops_old_samples() {
        let r = Recorder::default();
        let start = Instant::now();
        r.record_at(Stage::Retrieval, Duration::from_millis(50), start);
        let later = start + WINDOW + Duration::from_secs(1);
        r.record_at(Stage::Retrieval, Duration::from_millis(70), later);
        let s = r
            .snapshot_at(later)
            .into_iter()
            .find(|s| s.stage == Stage::Retrieval)
            .unwrap();
        assert_eq!((s.count, s.p50_ms), (1, Some(70)));
    }

    #[test]
    fn keeps_at_most_1000_samples() {
        let r = Recorder::default();
        for i in 0..1_500 {
            r.record(Stage::LlmFirstToken, Duration::from_millis(i));
        }
        let s = r
            .snapshot()
            .into_iter()
            .find(|s| s.stage == Stage::LlmFirstToken)
            .unwrap();
        assert_eq!(s.count, 1_000);
        assert_eq!(s.p50_ms, Some(999), "oldest 500 dropped");
    }

    #[test]
    fn serialises_stage_names_for_the_ui() {
        assert_eq!(
            serde_json::to_value(Stage::QuestionToCue).unwrap(),
            "question_to_cue"
        );
    }
}
