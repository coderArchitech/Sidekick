//! Declares the app's own IPC commands so each window must be granted them
//! explicitly in capabilities/*.json (tech doc 5.8). Add new commands here.

const COMMANDS: &[&str] = &["app_info"];

fn main() {
    #[allow(clippy::expect_used)] // Build script: failing loudly is the right behaviour.
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDS)),
    )
    .expect("tauri build step failed");
}
