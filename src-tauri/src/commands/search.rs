//! Web search (PRD KEY-8) for agent Skills' `web_search` tool (SKL-4).
//! Overlay window. Uses the provider chosen under Web search in Settings.

use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_gateway::SearchResult;
use tauri::State;

use crate::state::AppState;

/// Searches the web with the Settings provider; an error when web search is off.
#[tauri::command]
#[specta::specta]
pub async fn web_search(
    state: State<'_, AppState>,
    query: String,
) -> SidekickResult<Vec<SearchResult>> {
    let choice = sidekick_settings::load(&state.db())?
        .search
        .ok_or_else(|| {
            SidekickError::new(
                ErrorCode::Internal,
                "Web search is off. Choose Tavily or Brave under Web search in Settings.",
            )
        })?;
    let results = state.gateway.web_search(&choice.provider, &query).await?;
    tracing::info!(target: "gateway", provider = %choice.provider, results = results.len(), "web search");
    Ok(results)
}
