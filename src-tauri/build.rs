//! Declares the app's own IPC commands so each window must be granted them
//! explicitly in capabilities/*.json (tech doc 5.8). Add new commands here.

const COMMANDS: &[&str] = &[
    "app_info",
    "open_settings",
    "get_capture_protection",
    "get_settings",
    "update_settings",
    "list_providers",
    "set_api_key",
    "test_api_key",
    "delete_api_key",
    "list_models",
    "llm_stream",
    "cancel_stream",
    "get_diagnostics",
    "record_latency",
    "create_session",
    "web_search",
    "session_provider_options",
    "confirm_consent",
    "start_session",
    "pause_session",
    "resume_session",
    "end_session",
    "delete_session",
    "list_sessions",
    "get_transcript",
    "rename_speaker",
    "add_bookmark",
    "set_auto_answer",
    "save_answer",
    "rate_answer",
    "list_answers",
    "get_notes",
    "save_notes",
    "save_summary",
    "set_agenda",
    "update_agenda",
    "set_action_item_done",
    "capture_screenshot",
    "list_windows",
    "preview_screen",
    "list_screenshots",
    "screenshot_image",
    "delete_screenshot",
    "whisper_models",
    "download_whisper_model",
    "list_skills",
    "skill_templates",
    "check_skill",
    "save_skill",
    "set_skill_enabled",
    "delete_skill",
    "import_skill",
    "export_skill",
    "set_session_skill",
    "set_session_extra_skills",
    "set_overlay",
    "set_click_through",
    "get_click_through",
    "set_session_retention",
    "delete_all_data",
    "log",
    "export_logs",
    "ingest_document",
    "ingest_url",
    "list_documents",
    "delete_document",
    "create_pack",
    "rename_pack",
    "delete_pack",
    "list_packs",
    "list_pack_documents",
    "ingest_pack_document",
    "ingest_pack_url",
    "list_session_packs",
    "set_session_packs",
    "brief_material",
    "get_brief",
    "save_brief",
    "retrieve_context",
    "startup_problem",
    "recover_start_fresh",
    "recover_restore_backup",
    "recover_retry",
    "export_startup_logs",
    "quit_app",
];

fn main() {
    let mut attributes = tauri_build::Attributes::new()
        .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS));

    // On Windows, embed our own manifest into every binary (tests included)
    // instead of letting tauri-build add it to the app binary only.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        let manifest =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }

    #[allow(clippy::expect_used)] // Build script: failing loudly is the right behaviour.
    tauri_build::try_build(attributes).expect("tauri build step failed");
}
