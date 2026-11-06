//! Sidekick app shell.
//!
//! Responsibilities (tech doc 3.2, 7.8): build the Tauri app, register IPC
//! commands and events, and wire domain crates together. Commands are thin:
//! they validate input, call a domain crate, and map the result.

use tauri::Manager;

mod commands;
mod events;
mod housekeeping;
mod ipc;
mod logging;
mod placement;
mod protection;
mod shortcuts;
mod state;

#[cfg(test)]
mod capabilities_test;

/// Entry point shared by the desktop binary and (later) mobile targets.
pub fn run() {
    let builder = ipc::builder();

    let app = tauri::Builder::default();
    // E2E builds only (T-033): WebDriver inside the app, on every OS.
    #[cfg(feature = "e2e")]
    let app = app
        .plugin(tauri_plugin_wdio::init())
        .plugin(tauri_plugin_wdio_webdriver::init());

    #[allow(clippy::expect_used)] // Startup: the app cannot run without its context.
    app.plugin(shortcuts::plugin())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            logging::init(logging::log_dir(app.path().app_log_dir()?));
            tracing::info!(target: "app", version = env!("CARGO_PKG_VERSION"), os = std::env::consts::OS, "starting");
            builder.mount_events(app);
            let default_dir = app.path().app_data_dir()?;
            // A startup failure here should show the recovery screen (tech doc 4.5);
            // until that exists, fail with the error code in the log.
            let state = state::AppState::init(state::data_dir(default_dir)).map_err(|e| {
                tracing::error!(code = ?e.code, "startup failed");
                Box::new(e) as Box<dyn std::error::Error>
            })?;
            let bindings = sidekick_settings::load(&state.db())
                .map(|s| s.shortcuts)
                .unwrap_or_default();
            app.manage(state);
            app.manage(protection::apply(app.handle()));
            app.manage(shortcuts::Registered::default());
            app.manage(placement::Generation::default());
            placement::restore(app.handle());
            housekeeping::start(app.handle());
            if let Some(w) = app.get_webview_window("overlay") {
                let handle = app.handle().clone();
                w.on_window_event(move |e| {
                    if matches!(
                        e,
                        tauri::WindowEvent::Moved(_) | tauri::WindowEvent::Resized(_)
                    ) {
                        placement::on_changed(&handle);
                    }
                });
            }
            // A shortcut another app already owns shouldn't stop Sidekick from starting.
            if let Err(e) = shortcuts::apply(app.handle(), &bindings, &[]) {
                tracing::warn!(message = %e.message, "some shortcuts could not be registered");
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start Sidekick");
}
