//! Sidekick app shell.
//!
//! Responsibilities (tech doc 3.2, 7.8): build the Tauri app, register IPC
//! commands and events, and wire domain crates together. Commands are thin:
//! they validate input, call a domain crate, and map the result.

use tauri::Manager;

mod commands;
mod events;
mod ipc;
mod logging;
mod state;

#[cfg(test)]
mod capabilities_test;

/// Entry point shared by the desktop binary and (later) mobile targets.
pub fn run() {
    logging::init();
    let builder = ipc::builder();

    #[allow(clippy::expect_used)] // Startup: the app cannot run without its context.
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let default_dir = app.path().app_data_dir()?;
            // A startup failure here should show the recovery screen (tech doc 4.5);
            // until that exists, fail with the error code in the log.
            let state = state::AppState::init(state::data_dir(default_dir)).map_err(|e| {
                tracing::error!(code = ?e.code, "startup failed");
                Box::new(e) as Box<dyn std::error::Error>
            })?;
            app.manage(state);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start Sidekick");
}
