//! How long session data is kept after a session ends (tech doc 4.4).

use serde::{Deserialize, Serialize};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Retention {
    /// Deleted as soon as the session ends.
    #[serde(rename = "session")]
    SessionOnly,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "forever")]
    Forever,
}

impl Retention {
    /// The value stored in `sessions.retention`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SessionOnly => "session",
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
            Self::Forever => "forever",
        }
    }

    /// Unknown values keep data (never delete on a typo).
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "session" => Self::SessionOnly,
            "7d" => Self::SevenDays,
            "30d" => Self::ThirtyDays,
            _ => Self::Forever,
        }
    }

    /// When data of a session that ended at `ended_at` (epoch ms) expires.
    #[must_use]
    pub const fn expires_at(self, ended_at: i64) -> Option<i64> {
        match self {
            Self::SessionOnly => Some(ended_at),
            Self::SevenDays => Some(ended_at.saturating_add(7 * DAY_MS)),
            Self::ThirtyDays => Some(ended_at.saturating_add(30 * DAY_MS)),
            Self::Forever => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_off_per_option() {
        let ended = 1_000_000;
        assert_eq!(Retention::SessionOnly.expires_at(ended), Some(ended));
        assert_eq!(
            Retention::SevenDays.expires_at(ended),
            Some(ended + 7 * DAY_MS)
        );
        assert_eq!(
            Retention::ThirtyDays.expires_at(ended),
            Some(ended + 30 * DAY_MS)
        );
        assert_eq!(Retention::Forever.expires_at(ended), None);
        assert_eq!(
            Retention::SevenDays.expires_at(i64::MAX),
            Some(i64::MAX),
            "no overflow"
        );
    }

    #[test]
    fn stored_values_round_trip_and_unknown_keeps_data() {
        for r in [
            Retention::SessionOnly,
            Retention::SevenDays,
            Retention::ThirtyDays,
            Retention::Forever,
        ] {
            assert_eq!(Retention::parse(r.as_str()), r);
        }
        assert_eq!(Retention::parse("90d"), Retention::Forever);
    }
}
