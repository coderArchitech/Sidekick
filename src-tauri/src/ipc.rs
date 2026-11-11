//! The IPC contract: every command and event the frontend can use (tech doc 5.1, 5.2).
//!
//! `src/ipc/bindings.ts` is generated from this list by `pnpm ipc:gen`, so the
//! TypeScript types can never drift from the Rust signatures (tech doc 5.6).

use tauri_specta::{Builder, collect_commands, collect_events};

use crate::{commands, events};

/// Builds the typed command and event registry.
pub fn builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app::app_info,
            commands::app::open_settings,
            commands::app::quit_app,
            commands::app::get_capture_protection,
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::keys::list_providers,
            commands::keys::set_api_key,
            commands::keys::test_api_key,
            commands::keys::delete_api_key,
            commands::keys::list_models,
            commands::llm::llm_stream,
            commands::llm::cancel_stream,
            commands::diagnostics::get_diagnostics,
            commands::diagnostics::record_latency,
            commands::session::create_session,
            commands::search::web_search,
            commands::session::session_provider_options,
            commands::session::confirm_consent,
            commands::session::start_session,
            commands::session::pause_session,
            commands::session::resume_session,
            commands::session::end_session,
            commands::session::delete_session,
            commands::session::list_sessions,
            commands::session::get_transcript,
            commands::session::rename_speaker,
            commands::session::add_bookmark,
            commands::session::set_auto_answer,
            commands::session::set_my_transcription,
            commands::session::save_answer,
            commands::session::rate_answer,
            commands::session::list_answers,
            commands::notes::get_notes,
            commands::notes::save_notes,
            commands::notes::save_summary,
            commands::notes::set_agenda,
            commands::notes::update_agenda,
            commands::notes::set_action_item_done,
            commands::capture::capture_screenshot,
            commands::capture::list_windows,
            commands::capture::preview_screen,
            commands::capture::list_screenshots,
            commands::capture::screenshot_image,
            commands::capture::delete_screenshot,
            commands::stt::whisper_models,
            commands::stt::download_whisper_model,
            commands::skills::list_skills,
            commands::skills::skill_templates,
            commands::skills::check_skill,
            commands::skills::save_skill,
            commands::skills::set_skill_enabled,
            commands::skills::delete_skill,
            commands::skills::import_skill,
            commands::skills::export_skill,
            commands::skills::set_session_skill,
            commands::skills::set_session_extra_skills,
            commands::overlay::set_overlay,
            commands::overlay::set_click_through,
            commands::overlay::get_click_through,
            commands::data::set_session_retention,
            commands::data::delete_all_data,
            commands::diagnostics::log,
            commands::diagnostics::export_logs,
            commands::documents::ingest_document,
            commands::documents::ingest_url,
            commands::documents::list_documents,
            commands::documents::delete_document,
            commands::packs::create_pack,
            commands::packs::rename_pack,
            commands::packs::delete_pack,
            commands::packs::list_packs,
            commands::packs::list_pack_documents,
            commands::packs::ingest_pack_document,
            commands::packs::ingest_pack_url,
            commands::packs::list_session_packs,
            commands::packs::set_session_packs,
            commands::packs::brief_material,
            commands::packs::get_brief,
            commands::packs::save_brief,
            commands::retrieval::retrieve_context,
            commands::recovery::startup_problem,
            commands::recovery::recover_start_fresh,
            commands::recovery::recover_restore_backup,
            commands::recovery::recover_retry,
            commands::recovery::export_startup_logs,
        ])
        .events(collect_events![
            events::AnswerToken,
            events::AnswerDone,
            events::AnswerError,
            events::SessionChanged,
            events::TranscriptPartial,
            events::TranscriptSegment,
            events::SttStatusChanged,
            events::HotkeyPressed,
            events::IngestProgress,
            events::ScreenshotCaptured,
            events::ModelDownloadProgress,
            events::SettingsChanged,
            events::ClickThroughChanged,
        ])
}

#[cfg(test)]
mod tests {
    use specta_typescript::{BigIntExportBehavior, Typescript};

    /// Writes `src/ipc/bindings.ts`. Run through `pnpm ipc:gen`, which sets
    /// `SIDEKICK_IPC_GEN=1`; otherwise this test only checks the export works.
    #[test]
    fn export_typescript_bindings() {
        let target = if std::env::var("SIDEKICK_IPC_GEN").as_deref() == Ok("1") {
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/ipc/bindings.ts")
        } else {
            std::env::temp_dir().join("sidekick-bindings-check.ts")
        };
        super::builder()
            .export(
                Typescript::default()
                    .bigint(BigIntExportBehavior::Number)
                    .header("// @ts-nocheck\n// Generated by `pnpm ipc:gen` from src-tauri/src/ipc.rs. Do not edit.\n"),
                &target,
            )
            .expect("export bindings");
    }
}
