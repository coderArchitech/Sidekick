//! Typed events pushed to the frontend (tech doc 5.2). Tauri event names can't
//! contain dots, so `answer.token` is emitted as `answer-token`, and so on.

use serde::Serialize;
use sidekick_audio::StreamKind;
use sidekick_core::SidekickError;
use sidekick_core::llm::{FinishReason, Usage};
use sidekick_gateway::streams::AnswerSink;
use sidekick_session::Session;
use sidekick_stt::{Partial, Segment, SttSink, SttStatus};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerToken {
    pub request_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDone {
    pub request_id: String,
    pub usage: Usage,
    pub finish_reason: FinishReason,
}

#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AnswerError {
    pub request_id: String,
    pub error: SidekickError,
}

/// Sends gateway stream output to the frontend as events.
pub struct TauriAnswerSink(pub AppHandle);

impl AnswerSink for TauriAnswerSink {
    fn token(&self, request_id: &str, delta: &str) {
        let _ = AnswerToken {
            request_id: request_id.into(),
            delta: delta.into(),
        }
        .emit(&self.0);
    }
    fn done(&self, request_id: &str, usage: Usage, finish_reason: FinishReason) {
        let _ = AnswerDone {
            request_id: request_id.into(),
            usage,
            finish_reason,
        }
        .emit(&self.0);
    }
    fn error(&self, request_id: &str, error: &SidekickError) {
        let _ = AnswerError {
            request_id: request_id.into(),
            error: error.clone(),
        }
        .emit(&self.0);
    }
}

/// A session was created or changed status (`session.state`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SessionChanged {
    pub session: Session,
}

/// Which audio stream a transcript event belongs to.
#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum Stream {
    Mic,
    System,
}

impl From<StreamKind> for Stream {
    fn from(k: StreamKind) -> Self {
        match k {
            StreamKind::Mic => Self::Mic,
            StreamKind::System => Self::System,
        }
    }
}

/// Interim text, replaced by the next partial or a segment (`transcript.partial`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPartial {
    pub session_id: String,
    pub stream: Stream,
    pub text: String,
}

/// A finalised segment, already saved (`transcript.segment`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub session_id: String,
    pub id: i64,
    pub speaker_label: String,
    pub is_user: bool,
    pub text: String,
    pub start_ms: u32,
    pub end_ms: u32,
}

#[derive(Debug, Clone, Copy, Serialize, specta::Type)]
#[serde(rename_all = "snake_case")]
pub enum SttState {
    Connecting,
    Live,
    Reconnecting,
    Failed,
}

/// STT connection changes (`stt.status`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SttStatusChanged {
    pub session_id: String,
    pub stream: Stream,
    pub state: SttState,
}

/// Saves segments and forwards transcription to the frontend.
pub struct TauriSttSink {
    app: AppHandle,
    session_id: String,
}

impl TauriSttSink {
    pub fn new(app: AppHandle, session_id: String) -> Self {
        Self { app, session_id }
    }
}

impl SttSink for TauriSttSink {
    fn partial(&self, p: Partial) {
        let _ = TranscriptPartial {
            session_id: self.session_id.clone(),
            stream: p.kind.into(),
            text: p.text,
        }
        .emit(&self.app);
    }

    fn segment(&self, s: Segment) {
        let state = self.app.state::<crate::state::AppState>();
        let id = match sidekick_session::repo::insert_segment(&state.db(), &self.session_id, &s) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!(code = ?e.code, "couldn't save a transcript segment");
                return;
            }
        };
        let _ = TranscriptSegment {
            session_id: self.session_id.clone(),
            id,
            speaker_label: s.speaker_label,
            is_user: s.is_user,
            text: s.text,
            start_ms: u32::try_from(s.start_ms).unwrap_or(u32::MAX),
            end_ms: u32::try_from(s.end_ms).unwrap_or(u32::MAX),
        }
        .emit(&self.app);
    }

    fn status(&self, kind: StreamKind, status: SttStatus) {
        let state = match status {
            SttStatus::Connecting => SttState::Connecting,
            SttStatus::Live => SttState::Live,
            SttStatus::Reconnecting => SttState::Reconnecting,
            SttStatus::Failed => SttState::Failed,
        };
        let _ = SttStatusChanged {
            session_id: self.session_id.clone(),
            stream: kind.into(),
            state,
        }
        .emit(&self.app);
    }
}

/// A global shortcut was pressed (`hotkey.pressed`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct HotkeyPressed {
    pub action: sidekick_hotkeys::Action,
}

/// Settings were saved (`settings.changed`), so the other window can follow,
/// e.g. the theme (PRD UI-8).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SettingsChanged {
    pub settings: sidekick_settings::Settings,
}

/// Click-through was turned on or off (PRD UI-5).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ClickThroughChanged {
    pub on: bool,
}

/// Document ingest progress (`ingest.progress`).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct IngestProgress {
    #[serde(flatten)]
    pub progress: sidekick_ingest::Progress,
}

/// A screenshot was stored (`screenshot.captured`, tech doc 5.2).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ScreenshotCaptured {
    pub screenshot: sidekick_capture::Screenshot,
}

/// Whisper model download progress (Settings).
#[derive(Debug, Clone, Serialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub name: String,
    pub done_bytes: f64,
    pub total_bytes: Option<f64>,
}
