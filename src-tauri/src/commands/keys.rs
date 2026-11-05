//! Provider and API key commands (tech doc 5.1). Settings window only (5.8).
//! A key arrives once in `set_api_key`, is wrapped in `SecretString`
//! immediately, and is never sent back to the frontend.

use sidekick_core::SidekickResult;
use sidekick_core::llm::ModelInfo;
use sidekick_gateway::{KeyCheck, ProviderEntry, ProviderKind};
use sidekick_secrets::SecretString;
use tauri::State;

use crate::state::AppState;

#[tauri::command]
#[specta::specta]
pub fn list_providers(
    state: State<'_, AppState>,
    kind: ProviderKind,
) -> SidekickResult<Vec<ProviderEntry>> {
    state.gateway.providers(kind)
}

#[tauri::command]
#[specta::specta]
pub async fn set_api_key(
    state: State<'_, AppState>,
    provider: String,
    key: String,
) -> SidekickResult<KeyCheck> {
    state
        .gateway
        .set_key(&provider, SecretString::from(key))
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn test_api_key(
    state: State<'_, AppState>,
    provider: String,
) -> SidekickResult<KeyCheck> {
    state.gateway.test_key(&provider).await
}

#[tauri::command]
#[specta::specta]
pub fn delete_api_key(state: State<'_, AppState>, provider: String) -> SidekickResult<()> {
    state.gateway.delete_key(&provider)
}

#[tauri::command]
#[specta::specta]
pub async fn list_models(
    state: State<'_, AppState>,
    provider: String,
) -> SidekickResult<Vec<ModelInfo>> {
    state.gateway.list_models(&provider).await
}
