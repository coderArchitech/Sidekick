//! Sidekick app shell.
//!
//! Responsibilities (tech doc 3.2, 7.8): build the Tauri app, register IPC
//! commands and events, and wire domain crates together. Commands are thin:
//! they validate input, call a domain crate, and map the result.

mod commands;
mod ipc;
mod logging;

/// Entry point shared by the desktop binary and (later) mobile targets.
pub fn run() {
    logging::init();
    let builder = ipc::builder();

    #[allow(clippy::expect_used)] // Startup: the app cannot run without its context.
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("failed to start Sidekick");
}
