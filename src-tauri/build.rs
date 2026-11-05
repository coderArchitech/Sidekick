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
        println!("cargo:rustc-link-arg=/WX");
    }

    #[allow(clippy::expect_used)] // Build script: failing loudly is the right behaviour.
    tauri_build::try_build(attributes).expect("tauri build step failed");
}
