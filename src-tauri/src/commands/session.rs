//! Session commands (tech doc 5.1): create, consent, start, pause, resume, end,
//! delete, list. Overlay window. Listening to others never starts without
//! recorded consent (PRD 8); "Transcribe me only" starts the microphone only.

use std::sync::Arc;

use serde::Serialize;
use sidekick_audio::{Pacing, StreamKind};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_session::model::{Action, transition};
use sidekick_session::repo::{self, NewSession, SessionPage};
use sidekick_session::{
    AudioFactory, ConsentOption, DeviceAudio, FixtureAudio, LiveSession, Session, SessionStatus,
    SttPlan, streams_for,
};
use sidekick_settings::ProviderChoice;
use sidekick_stt::{DeepgramProvider, MockSttProvider};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use crate::events::{SessionChanged, TauriSttSink};
use crate::state::AppState;

/// Result of starting a session: the session plus non-fatal warnings to show.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionStarted {
    pub session: Session,
    pub warnings: Vec<String>,
}

pub(crate) fn changed(app: &AppHandle, session: &Session) {
    let _ = SessionChanged {
        session: session.clone(),
    }
    .emit(app);
}

#[tauri::command]
#[specta::specta]
pub fn create_session(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
) -> SidekickResult<Session> {
    let db = state.db();
    let settings = sidekick_settings::load(&db)?;
    let session = repo::create(
        &db,
        &NewSession {
            title: &title,
            stt_provider: &settings.stt.provider,
            llm_provider: &settings.llm.provider,
            llm_model: settings
                .llm
                .model
                .as_deref()
                .unwrap_or(sidekick_gateway::DEFAULT_ANSWER_MODEL),
            auto_answer: settings.auto_answer,
        },
    )?;
    changed(&app, &session);
    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub fn confirm_consent(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    option: ConsentOption,
) -> SidekickResult<Session> {
    let session = repo::record_consent(&state.db(), &session_id, option)?;
    changed(&app, &session);
    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<SessionStarted> {
    let mut live = state.live.lock().await;
    if live.is_some() {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            "End the current session before starting another.",
        ));
    }
    let (session, settings) = {
        let db = state.db();
        (repo::get(&db, &session_id)?, sidekick_settings::load(&db)?)
    };
    transition(session.status, Action::Start)?;
    let mut kinds = streams_for(session.consent)?;

    let fixture = std::env::var_os("SIDEKICK_AUDIO_FIXTURE");
    let audio: Box<dyn AudioFactory> = match &fixture {
        Some(path) => {
            // One file plays as one stream, so it isn't transcribed twice.
            kinds = vec![if kinds.contains(&StreamKind::System) {
                StreamKind::System
            } else {
                StreamKind::Mic
            }];
            Box::new(FixtureAudio {
                path: path.into(),
                pacing: Pacing::RealTime,
            })
        }
        None => Box::new(DeviceAudio),
    };
    let mut plan = stt_plan(&state, &settings.stt)?;
    // Boost the user's name and nicknames so directed questions are heard (PRD LIS-5a).
    plan.keyterms = settings.profile.names();
    let sink = Arc::new(TauriSttSink::new(app.clone(), session_id.clone()));
    let runner = LiveSession::start(&kinds, audio.as_ref(), &plan, sink)?;
    let warnings = runner.warnings.clone();
    let session = repo::set_status(&state.db(), &session_id, SessionStatus::Live)?;
    *live = Some((session_id, runner));
    changed(&app, &session);
    Ok(SessionStarted { session, warnings })
}

fn stt_plan(state: &AppState, choice: &ProviderChoice) -> SidekickResult<SttPlan> {
    let key_for = |id: &str, name: &str| {
        state.keys.get(id)?.ok_or_else(|| {
            SidekickError::new(
                ErrorCode::KeyMissing,
                format!("Add your {name} API key in Settings."),
            )
        })
    };
    let (provider, key): (Arc<dyn sidekick_stt::SttProvider>, _) = if state.mock {
        (Arc::new(MockSttProvider), None)
    } else {
        match choice.provider.as_str() {
            "deepgram" => (
                Arc::new(DeepgramProvider::default()),
                Some(key_for("deepgram", "Deepgram")?),
            ),
            "assemblyai" => (
                Arc::new(sidekick_stt::providers::assemblyai::AssemblyAiProvider::default()),
                Some(key_for("assemblyai", "AssemblyAI")?),
            ),
            "soniox" => (
                Arc::new(sidekick_stt::providers::soniox::SonioxProvider::default()),
                Some(key_for("soniox", "Soniox")?),
            ),
            #[cfg(feature = "local-whisper")]
            "whisper-local" => {
                let model = crate::commands::stt::whisper_model(choice.model.as_deref())?;
                let path =
                    sidekick_stt::models::path(&crate::commands::stt::models_dir(state), model);
                (
                    Arc::new(sidekick_stt::providers::whisper::WhisperProvider::new(path)),
                    None,
                )
            }
            other => {
                return Err(SidekickError::new(
                    ErrorCode::Internal,
                    format!(
                        "Speech-to-text with {other} isn't available in this build. Choose another provider in Settings."
                    ),
                ));
            }
        }
    };
    Ok(SttPlan {
        provider,
        key,
        language: "en".into(),
        keyterms: Vec::new(),
    })
}

async fn change_live(
    app: &AppHandle,
    state: &AppState,
    session_id: &str,
    action: Action,
) -> SidekickResult<Session> {
    let live = state.live.lock().await;
    let current = repo::get(&state.db(), session_id)?;
    let next = transition(current.status, action)?;
    match live.as_ref() {
        Some((id, runner)) if id == session_id => match action {
            Action::Pause => runner.pause(),
            Action::Resume => runner.resume(),
            _ => {}
        },
        _ => return Err(SidekickError::internal("This session isn't running.")),
    }
    let session = repo::set_status(&state.db(), session_id, next)?;
    changed(app, &session);
    Ok(session)
}

#[tauri::command]
#[specta::specta]
pub async fn pause_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Session> {
    change_live(&app, &state, &session_id, Action::Pause).await
}

#[tauri::command]
#[specta::specta]
pub async fn resume_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Session> {
    change_live(&app, &state, &session_id, Action::Resume).await
}

/// Stops listening and lets the transcript finish. The summary arrives with T-024.
#[tauri::command]
#[specta::specta]
pub async fn end_session(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Session> {
    let current = repo::get(&state.db(), &session_id)?;
    transition(current.status, Action::End)?;
    stop_if_live(&state, &session_id).await;
    let session = repo::set_status(&state.db(), &session_id, SessionStatus::Ended)?;
    changed(&app, &session);
    Ok(session)
}

/// Deletes everything in the session (the consent record is kept, tech doc 4.4).
#[tauri::command]
#[specta::specta]
pub async fn delete_session(state: State<'_, AppState>, session_id: String) -> SidekickResult<()> {
    stop_if_live(&state, &session_id).await;
    repo::delete(&state.db(), &session_id)
}

#[tauri::command]
#[specta::specta]
pub fn list_sessions(
    state: State<'_, AppState>,
    cursor: Option<String>,
    limit: Option<u32>,
) -> SidekickResult<SessionPage> {
    repo::list(&state.db(), cursor.as_deref(), limit)
}

async fn stop_if_live(state: &AppState, session_id: &str) {
    let mut live = state.live.lock().await;
    if live.as_ref().is_some_and(|(id, _)| id == session_id)
        && let Some((_, runner)) = live.take()
    {
        drop(live);
        runner.stop().await;
    }
}

/// Everything the Transcript tab needs when it opens (T-014).
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Transcript {
    pub lines: Vec<repo::TranscriptLine>,
    pub bookmarks: Vec<repo::Bookmark>,
}

#[tauri::command]
#[specta::specta]
pub fn get_transcript(
    state: State<'_, AppState>,
    session_id: String,
) -> SidekickResult<Transcript> {
    let db = state.db();
    Ok(Transcript {
        lines: repo::list_segments(&db, &session_id)?,
        bookmarks: repo::list_bookmarks(&db, &session_id)?,
    })
}

/// Renames (or, if the name exists, merges) a speaker across the session.
#[tauri::command]
#[specta::specta]
pub fn rename_speaker(
    state: State<'_, AppState>,
    session_id: String,
    from: String,
    to: String,
) -> SidekickResult<u32> {
    repo::rename_speaker(&state.db(), &session_id, &from, &to)
        .map(|n| u32::try_from(n).unwrap_or(u32::MAX))
}

/// Marks a moment; `at_ms` is milliseconds from session start.
#[tauri::command]
#[specta::specta]
pub fn add_bookmark(
    state: State<'_, AppState>,
    session_id: String,
    at_ms: u32,
    label: Option<String>,
) -> SidekickResult<repo::Bookmark> {
    repo::add_bookmark(&state.db(), &session_id, at_ms, label.as_deref())
}

/// Auto-answer on/off for the session (PRD QA-3: header button and hotkey).
#[tauri::command]
#[specta::specta]
pub fn set_auto_answer(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    on: bool,
) -> SidekickResult<Session> {
    let session = repo::set_auto_answer(&state.db(), &session_id, on)?;
    changed(&app, &session);
    Ok(session)
}

/// Saves a finished answer with its question (tech doc 5.1 `save_answer`).
#[tauri::command]
#[specta::specta]
pub fn save_answer(
    state: State<'_, AppState>,
    answer: sidekick_session::answers::NewAnswer,
) -> SidekickResult<String> {
    sidekick_session::answers::save(&mut state.db(), &answer)
}

/// Thumbs up (1) / down (-1) / clear (0).
#[tauri::command]
#[specta::specta]
pub fn rate_answer(
    state: State<'_, AppState>,
    answer_id: String,
    rating: i8,
) -> SidekickResult<()> {
    sidekick_session::answers::rate(&state.db(), &answer_id, rating)
}

/// A session's saved answers, oldest first, optionally one source only
/// (Chat history, PRD QA-8).
#[tauri::command]
#[specta::specta]
pub fn list_answers(
    state: State<'_, AppState>,
    session_id: String,
    source: Option<sidekick_session::answers::QuestionSource>,
) -> SidekickResult<Vec<sidekick_session::answers::SavedAnswer>> {
    sidekick_session::answers::list(&state.db(), &session_id, source)
}
