//! Typed settings (tech doc 4.5 defaults, 5.1 `get_settings` / `update_settings`).
//!
//! Each top-level section is stored as one JSON row in the `settings` table
//! under its section name (`stt`, `llm`, ...). Updates are partial: a patch
//! replaces only the sections it names. API keys never live here (ADR-008).

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};
use sidekick_hotkeys::Binding;
use sidekick_store::{Database, settings as raw};

/// The active provider and model for one kind of service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderChoice {
    /// Provider id, e.g. `deepgram`, `anthropic`, `openai`.
    pub provider: String,
    /// Model id, or `None` to use the provider's default.
    pub model: Option<String>,
}

/// How long session data is kept after a session ends (tech doc 4.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum Retention {
    #[serde(rename = "session")]
    SessionOnly,
    #[serde(rename = "7d")]
    SevenDays,
    #[serde(rename = "30d")]
    ThirtyDays,
    #[serde(rename = "forever")]
    Forever,
}

/// All user settings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub stt: ProviderChoice,
    pub llm: ProviderChoice,
    pub embeddings: ProviderChoice,
    /// Web search is optional; `None` means disabled.
    pub search: Option<ProviderChoice>,
    pub auto_answer: bool,
    pub retention: Retention,
    /// Global shortcuts, one per action (tech doc 12.3).
    pub shortcuts: Vec<Binding>,
}

impl Default for Settings {
    /// Defaults from tech doc 4.5 and ADR-013.
    fn default() -> Self {
        Self {
            stt: ProviderChoice {
                provider: "deepgram".into(),
                model: Some("nova-3".into()),
            },
            llm: ProviderChoice {
                provider: "anthropic".into(),
                model: None,
            },
            embeddings: ProviderChoice {
                provider: "openai".into(),
                model: Some("text-embedding-3-small".into()),
            },
            search: None,
            auto_answer: true,
            retention: Retention::ThirtyDays,
            shortcuts: sidekick_hotkeys::defaults(),
        }
    }
}

/// A partial update. Only the sections present are replaced; unknown fields are
/// rejected so typos don't silently do nothing.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SettingsPatch {
    #[serde(default)]
    pub stt: Option<ProviderChoice>,
    #[serde(default)]
    pub llm: Option<ProviderChoice>,
    #[serde(default)]
    pub embeddings: Option<ProviderChoice>,
    /// Omit to leave unchanged; `null` disables web search.
    #[serde(default, with = "double_option")]
    #[cfg_attr(feature = "specta", specta(optional))]
    pub search: Option<Option<ProviderChoice>>,
    #[serde(default)]
    pub auto_answer: Option<bool>,
    #[serde(default)]
    pub retention: Option<Retention>,
    /// The full set of shortcuts; validated (every action once, no duplicates).
    #[serde(default)]
    pub shortcuts: Option<Vec<Binding>>,
}

impl Settings {
    /// Applies a patch and validates the result.
    pub fn apply(mut self, patch: SettingsPatch) -> SidekickResult<Self> {
        if let Some(v) = patch.stt {
            self.stt = v;
        }
        if let Some(v) = patch.llm {
            self.llm = v;
        }
        if let Some(v) = patch.embeddings {
            self.embeddings = v;
        }
        if let Some(v) = patch.search {
            self.search = v;
        }
        if let Some(v) = patch.auto_answer {
            self.auto_answer = v;
        }
        if let Some(v) = patch.retention {
            self.retention = v;
        }
        if let Some(v) = patch.shortcuts {
            self.shortcuts = sidekick_hotkeys::validate(&v)?;
        }
        self.validate()?;
        Ok(self)
    }

    fn validate(&self) -> SidekickResult<()> {
        let choices = [
            Some(&self.stt),
            Some(&self.llm),
            Some(&self.embeddings),
            self.search.as_ref(),
        ];
        for choice in choices.into_iter().flatten() {
            let id = &choice.provider;
            let valid = !id.is_empty()
                && id.len() <= 32
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            if !valid {
                return Err(SidekickError::new(
                    ErrorCode::Internal,
                    "Invalid provider id in settings.",
                ));
            }
        }
        Ok(())
    }
}

/// Loads settings, filling any missing or unreadable section with its default.
pub fn load(db: &Database) -> SidekickResult<Settings> {
    let d = Settings::default();
    Ok(Settings {
        stt: section(db, "stt")?.unwrap_or(d.stt),
        llm: section(db, "llm")?.unwrap_or(d.llm),
        embeddings: section(db, "embeddings")?.unwrap_or(d.embeddings),
        search: section(db, "search")?.unwrap_or(d.search),
        auto_answer: section(db, "autoAnswer")?.unwrap_or(d.auto_answer),
        retention: section(db, "retention")?.unwrap_or(d.retention),
        shortcuts: section::<Vec<Binding>>(db, "shortcuts")?
            .and_then(|b| sidekick_hotkeys::validate(&b).ok())
            .unwrap_or(d.shortcuts),
    })
}

/// Applies a patch, saves the changed sections and returns the new settings.
pub fn update(db: &Database, patch: SettingsPatch) -> SidekickResult<Settings> {
    let updated = load(db)?.apply(patch.clone())?;
    if patch.stt.is_some() {
        raw::set(db, "stt", &to_json(&updated.stt)?)?;
    }
    if patch.llm.is_some() {
        raw::set(db, "llm", &to_json(&updated.llm)?)?;
    }
    if patch.embeddings.is_some() {
        raw::set(db, "embeddings", &to_json(&updated.embeddings)?)?;
    }
    if patch.search.is_some() {
        raw::set(db, "search", &to_json(&updated.search)?)?;
    }
    if patch.auto_answer.is_some() {
        raw::set(db, "autoAnswer", &to_json(&updated.auto_answer)?)?;
    }
    if patch.retention.is_some() {
        raw::set(db, "retention", &to_json(&updated.retention)?)?;
    }
    if patch.shortcuts.is_some() {
        raw::set(db, "shortcuts", &to_json(&updated.shortcuts)?)?;
    }
    Ok(updated)
}

fn section<T: DeserializeOwned>(db: &Database, key: &str) -> SidekickResult<Option<T>> {
    // A section that no longer matches the current shape falls back to its
    // default instead of breaking startup (tech doc 5.5, deprecation).
    Ok(raw::get(db, key)?.and_then(|v| serde_json::from_value(v).ok()))
}

fn to_json<T: Serialize>(value: &T) -> SidekickResult<serde_json::Value> {
    serde_json::to_value(value).map_err(|_| SidekickError::internal("Couldn't save settings."))
}

/// Distinguishes "field absent" (leave unchanged) from "field is null" (clear).
mod double_option {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[allow(clippy::ref_option, clippy::option_option)]
    pub fn serialize<T: Serialize, S: Serializer>(
        v: &Option<Option<T>>,
        s: S,
    ) -> Result<S::Ok, S::Error> {
        match v {
            Some(inner) => inner.serialize(s),
            None => s.serialize_none(),
        }
    }

    #[allow(clippy::option_option)]
    pub fn deserialize<'de, T: Deserialize<'de>, D: Deserializer<'de>>(
        d: D,
    ) -> Result<Option<Option<T>>, D::Error> {
        Option::<T>::deserialize(d).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_secrets::SecretString;

    fn db() -> Database {
        Database::open_in_memory(&SecretString::from("d".repeat(64))).unwrap()
    }

    #[test]
    fn defaults_match_the_tech_doc() {
        let s = Settings::default();
        assert_eq!(s.stt.provider, "deepgram");
        assert!(s.auto_answer);
        assert_eq!(s.retention, Retention::ThirtyDays);
        assert_eq!(
            s.embeddings.model.as_deref(),
            Some("text-embedding-3-small")
        );
        assert!(s.search.is_none());
    }

    #[test]
    fn empty_database_loads_defaults() {
        assert_eq!(load(&db()).unwrap(), Settings::default());
    }

    #[test]
    fn partial_patch_changes_only_named_sections() {
        let db = db();
        let patch: SettingsPatch = serde_json::from_value(serde_json::json!({
            "autoAnswer": false,
            "llm": { "provider": "openai", "model": "gpt-5" }
        }))
        .unwrap();
        let updated = update(&db, patch).unwrap();
        assert!(!updated.auto_answer);
        assert_eq!(updated.llm.provider, "openai");
        assert_eq!(updated.stt, Settings::default().stt, "untouched");
        assert_eq!(load(&db).unwrap(), updated, "persisted");
    }

    #[test]
    fn search_can_be_set_and_cleared() {
        let db = db();
        let set: SettingsPatch = serde_json::from_value(
            serde_json::json!({ "search": { "provider": "tavily", "model": null } }),
        )
        .unwrap();
        assert!(update(&db, set).unwrap().search.is_some());
        let untouched: SettingsPatch =
            serde_json::from_value(serde_json::json!({ "autoAnswer": true })).unwrap();
        assert!(
            update(&db, untouched).unwrap().search.is_some(),
            "absent field leaves it"
        );
        let clear: SettingsPatch =
            serde_json::from_value(serde_json::json!({ "search": null })).unwrap();
        assert!(
            update(&db, clear).unwrap().search.is_none(),
            "null clears it"
        );
    }

    #[test]
    fn shortcuts_default_validate_and_persist() {
        let db = db();
        assert_eq!(load(&db).unwrap().shortcuts, sidekick_hotkeys::defaults());
        let mut keys = sidekick_hotkeys::defaults();
        keys[0].keys = "alt+shift+o".into();
        let updated = update(
            &db,
            SettingsPatch {
                shortcuts: Some(keys),
                ..SettingsPatch::default()
            },
        )
        .unwrap();
        assert_eq!(updated.shortcuts[0].keys, "Alt+Shift+O", "normalised");
        assert_eq!(load(&db).unwrap().shortcuts[0].keys, "Alt+Shift+O");

        let mut dup = sidekick_hotkeys::defaults();
        dup[1].keys = dup[0].keys.clone();
        assert!(
            update(
                &db,
                SettingsPatch {
                    shortcuts: Some(dup),
                    ..SettingsPatch::default()
                }
            )
            .is_err()
        );
        assert_eq!(
            load(&db).unwrap().shortcuts[0].keys,
            "Alt+Shift+O",
            "invalid patch changes nothing"
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let bad =
            serde_json::from_value::<SettingsPatch>(serde_json::json!({ "autoAnswr": false }));
        assert!(bad.is_err());
    }

    #[test]
    fn invalid_provider_ids_are_rejected() {
        let patch = SettingsPatch {
            llm: Some(ProviderChoice {
                provider: "Not Valid!".into(),
                model: None,
            }),
            ..SettingsPatch::default()
        };
        assert!(update(&db(), patch).is_err());
    }

    #[test]
    fn retention_uses_the_documented_wire_values() {
        let values: Vec<String> = [
            Retention::SessionOnly,
            Retention::SevenDays,
            Retention::ThirtyDays,
            Retention::Forever,
        ]
        .iter()
        .map(|r| {
            serde_json::to_value(r)
                .unwrap()
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect();
        assert_eq!(values, ["session", "7d", "30d", "forever"]);
    }

    #[test]
    fn corrupt_section_falls_back_to_default() {
        let db = db();
        raw::set(&db, "llm", &serde_json::json!({ "unexpected": 1 })).unwrap();
        assert_eq!(load(&db).unwrap().llm, Settings::default().llm);
    }

    #[test]
    fn settings_persist_across_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sidekick.db");
        let key = SecretString::from("e".repeat(64));
        {
            let db = Database::open(&path, &key).unwrap();
            update(
                &db,
                SettingsPatch {
                    auto_answer: Some(false),
                    ..SettingsPatch::default()
                },
            )
            .unwrap();
        }
        let db = Database::open(&path, &key).unwrap();
        assert!(!load(&db).unwrap().auto_answer);
    }
}
