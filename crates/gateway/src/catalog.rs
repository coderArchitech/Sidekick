//! Every provider Sidekick knows about, for the Settings dropdowns (PRD KEY-1 to KEY-3a, KEY-8).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "lowercase")]
pub enum ProviderKind {
    Stt,
    Llm,
    Embed,
    Search,
}

/// Static facts about a provider. `implemented` is false until its adapter lands
/// (non-LLM providers are also marked implemented once a key verifier is registered).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: &'static str,
    pub kind: ProviderKind,
    pub display_name: &'static str,
    pub key_required: bool,
    pub implemented: bool,
    pub docs_url: &'static str,
}

const fn p(
    id: &'static str,
    kind: ProviderKind,
    display_name: &'static str,
    key_required: bool,
    implemented: bool,
    docs_url: &'static str,
) -> ProviderInfo {
    ProviderInfo {
        id,
        kind,
        display_name,
        key_required,
        implemented,
        docs_url,
    }
}

use ProviderKind::{Embed, Llm, Search, Stt};

/// The catalog. The first entry of each kind is that kind's default.
pub const PROVIDERS: &[ProviderInfo] = &[
    p(
        "deepgram",
        Stt,
        "Deepgram",
        true,
        true,
        "https://developers.deepgram.com",
    ),
    p(
        "assemblyai",
        Stt,
        "AssemblyAI",
        true,
        false,
        "https://www.assemblyai.com/docs",
    ),
    p(
        "soniox",
        Stt,
        "Soniox",
        true,
        false,
        "https://soniox.com/docs",
    ),
    p(
        "whisper-local",
        Stt,
        "Local Whisper",
        false,
        false,
        "https://github.com/ggml-org/whisper.cpp",
    ),
    p(
        "anthropic",
        Llm,
        "Anthropic (Claude)",
        true,
        true,
        "https://platform.claude.com/docs",
    ),
    p(
        "openai",
        Llm,
        "OpenAI",
        true,
        true,
        "https://platform.openai.com/docs",
    ),
    p(
        "gemini",
        Llm,
        "Google Gemini",
        true,
        true,
        "https://ai.google.dev/gemini-api/docs",
    ),
    p(
        "ollama",
        Llm,
        "Ollama (local)",
        false,
        true,
        "https://ollama.com",
    ),
    p(
        "openai",
        Embed,
        "OpenAI",
        true,
        true,
        "https://platform.openai.com/docs",
    ),
    p(
        "gemini",
        Embed,
        "Google Gemini",
        true,
        false,
        "https://ai.google.dev/gemini-api/docs",
    ),
    p(
        "voyage",
        Embed,
        "Voyage AI",
        true,
        false,
        "https://docs.voyageai.com",
    ),
    p(
        "ollama",
        Embed,
        "Ollama (local)",
        false,
        false,
        "https://ollama.com",
    ),
    p(
        "tavily",
        Search,
        "Tavily",
        true,
        true,
        "https://docs.tavily.com",
    ),
    p(
        "brave",
        Search,
        "Brave Search",
        true,
        true,
        "https://brave.com/search/api",
    ),
];

/// Providers of one kind, default first.
pub fn of_kind(kind: ProviderKind) -> Vec<ProviderInfo> {
    PROVIDERS
        .iter()
        .filter(|p| p.kind == kind)
        .cloned()
        .collect()
}

/// Whether any kind of provider with this id needs a key. Keys are stored per
/// provider id, so OpenAI's key serves both LLM and embeddings (PRD KEY-3a).
pub fn key_required(id: &str) -> Option<bool> {
    let matches: Vec<_> = PROVIDERS.iter().filter(|p| p.id == id).collect();
    if matches.is_empty() {
        None
    } else {
        Some(matches.iter().any(|p| p.key_required))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_come_first() {
        assert_eq!(of_kind(Stt)[0].id, "deepgram");
        assert_eq!(of_kind(Llm)[0].id, "anthropic");
        assert_eq!(of_kind(Embed)[0].id, "openai");
    }

    #[test]
    fn local_providers_need_no_key() {
        assert_eq!(key_required("ollama"), Some(false));
        assert_eq!(key_required("whisper-local"), Some(false));
        assert_eq!(key_required("openai"), Some(true));
        assert_eq!(key_required("nope"), None);
    }

    #[test]
    fn ids_are_valid_keychain_accounts() {
        for p in PROVIDERS {
            assert!(sidekick_secrets::validate_account(p.id).is_ok(), "{}", p.id);
        }
    }
}
