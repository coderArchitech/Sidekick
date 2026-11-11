//! Session lifecycle and consent rules (PRD section 8).

use serde::{Deserialize, Serialize};
use sidekick_audio::StreamKind;
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Draft,
    Live,
    Paused,
    Ended,
}

impl SessionStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Live => "live",
            Self::Paused => "paused",
            Self::Ended => "ended",
        }
    }

    pub fn parse(s: &str) -> SidekickResult<Self> {
        match s {
            "draft" => Ok(Self::Draft),
            "live" => Ok(Self::Live),
            "paused" => Ok(Self::Paused),
            "ended" => Ok(Self::Ended),
            _ => Err(SidekickError::new(
                ErrorCode::DbError,
                "Unknown session status.",
            )),
        }
    }
}

/// What the user confirmed before listening started (PRD 8, "Session start confirmation").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum ConsentOption {
    /// Participants are aware of and agree to transcription: mic and system audio.
    ParticipantsAgreed,
    /// "Transcribe me only": the microphone only, never other participants.
    MeOnly,
    /// "Transcribe the other people only": system audio only, never the
    /// microphone. The other participants agree to transcription.
    OthersOnly,
}

impl ConsentOption {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParticipantsAgreed => "participants_agreed",
            Self::MeOnly => "me_only",
            Self::OthersOnly => "others_only",
        }
    }

    pub fn parse(s: &str) -> SidekickResult<Self> {
        match s {
            "participants_agreed" => Ok(Self::ParticipantsAgreed),
            "me_only" => Ok(Self::MeOnly),
            "others_only" => Ok(Self::OthersOnly),
            _ => Err(SidekickError::new(
                ErrorCode::DbError,
                "Unknown consent option.",
            )),
        }
    }
}

/// A session as the UI sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub title: String,
    pub status: SessionStatus,
    pub auto_answer: bool,
    /// Unix ms.
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    /// The consent recorded for this session, if any.
    pub consent: Option<ConsentOption>,
    /// The main Skill, if any (PRD SKL-1); the dropdown and hotkey switch it.
    pub skill_id: Option<String>,
    /// Skills also in use alongside the main one, in order (PRD SKL-5).
    pub extra_skill_ids: Vec<String>,
    /// Speech-to-text and answer providers for this session: the Settings
    /// defaults, or the ones picked when it was created (PRD KEY-9).
    pub stt_provider: String,
    pub llm_provider: String,
    /// How long its data is kept after it ends (tech doc 4.4).
    pub retention: sidekick_core::retention::Retention,
}

/// Which lifecycle moves are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Start,
    Pause,
    Resume,
    End,
}

/// Returns the status after `action`, or an error if the move isn't allowed.
pub fn transition(from: SessionStatus, action: Action) -> SidekickResult<SessionStatus> {
    use SessionStatus::{Draft, Ended, Live, Paused};
    let to = match (from, action) {
        (Draft, Action::Start) => Live,
        (Live, Action::Pause) => Paused,
        (Paused, Action::Resume) => Live,
        (Draft | Live | Paused, Action::End) => Ended,
        _ => {
            return Err(SidekickError::new(
                ErrorCode::Internal,
                format!(
                    "Can't {} a session that is {}.",
                    verb(action),
                    from.as_str()
                ),
            ));
        }
    };
    Ok(to)
}

fn verb(action: Action) -> &'static str {
    match action {
        Action::Start => "start",
        Action::Pause => "pause",
        Action::Resume => "resume",
        Action::End => "end",
    }
}

/// The audio streams a consent option allows. Without consent nothing may start.
pub fn streams_for(consent: Option<ConsentOption>) -> SidekickResult<Vec<StreamKind>> {
    match consent {
        Some(ConsentOption::ParticipantsAgreed) => Ok(vec![StreamKind::Mic, StreamKind::System]),
        Some(ConsentOption::MeOnly) => Ok(vec![StreamKind::Mic]),
        Some(ConsentOption::OthersOnly) => Ok(vec![StreamKind::System]),
        None => Err(SidekickError::new(
            ErrorCode::ConsentRequired,
            "Choose who Sidekick transcribes: the other people, the other people and you, or only you.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use SessionStatus::*;

    #[test]
    fn allowed_transitions() {
        assert_eq!(transition(Draft, Action::Start).unwrap(), Live);
        assert_eq!(transition(Live, Action::Pause).unwrap(), Paused);
        assert_eq!(transition(Paused, Action::Resume).unwrap(), Live);
        for s in [Draft, Live, Paused] {
            assert_eq!(transition(s, Action::End).unwrap(), Ended);
        }
    }

    #[test]
    fn rejected_transitions() {
        for (from, action) in [
            (Live, Action::Start),
            (Paused, Action::Start),
            (Ended, Action::Start),
            (Draft, Action::Pause),
            (Paused, Action::Pause),
            (Draft, Action::Resume),
            (Live, Action::Resume),
            (Ended, Action::End),
            (Ended, Action::Resume),
        ] {
            assert!(transition(from, action).is_err(), "{from:?} {action:?}");
        }
    }

    #[test]
    fn consent_decides_the_streams() {
        assert_eq!(
            streams_for(Some(ConsentOption::ParticipantsAgreed)).unwrap(),
            [StreamKind::Mic, StreamKind::System]
        );
        assert_eq!(
            streams_for(Some(ConsentOption::MeOnly)).unwrap(),
            [StreamKind::Mic]
        );
        assert_eq!(
            streams_for(Some(ConsentOption::OthersOnly)).unwrap(),
            [StreamKind::System]
        );
        assert_eq!(
            streams_for(None).unwrap_err().code,
            ErrorCode::ConsentRequired
        );
    }

    #[test]
    fn wire_values_round_trip() {
        for s in [Draft, Live, Paused, Ended] {
            assert_eq!(SessionStatus::parse(s.as_str()).unwrap(), s);
        }
        for c in [
            ConsentOption::ParticipantsAgreed,
            ConsentOption::MeOnly,
            ConsentOption::OthersOnly,
        ] {
            assert_eq!(ConsentOption::parse(c.as_str()).unwrap(), c);
        }
    }
}
