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

pub use sidekick_core::retention::Retention;

/// Who the user is, for directed questions and STT boosts (PRD QA-2, LIS-5a).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub name: String,
    pub nicknames: Vec<String>,
}

impl Profile {
    /// Name and nicknames, trimmed, without blanks or duplicates (case-insensitive).
    #[must_use]
    pub fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for n in std::iter::once(&self.name).chain(&self.nicknames) {
            let n = n.trim();
            if !n.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(n)) {
                out.push(n.to_owned());
            }
        }
        out
    }

    fn validate(&self) -> SidekickResult<()> {
        if self.nicknames.len() > 10 || self.names().iter().any(|n| n.chars().count() > 40) {
            return Err(SidekickError::new(
                ErrorCode::Internal,
                "Use up to 10 nicknames of up to 40 characters.",
            ));
        }
        Ok(())
    }
}

/// How the overlay looks and where it sits (PRD UI-1 to UI-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct OverlayPrefs {
    /// Percent, 20 to 100.
    pub opacity: u8,
    /// Text size in px, 12 to 20.
    pub font_size: u8,
    pub compact: bool,
    /// One per monitor, most recent first (managed by the app shell).
    pub positions: Vec<sidekick_window::placement::SavedPosition>,
}

impl Default for OverlayPrefs {
    fn default() -> Self {
        Self {
            opacity: 80,
            font_size: 14,
            compact: false,
            positions: Vec::new(),
        }
    }
}

impl OverlayPrefs {
    pub const OPACITY: std::ops::RangeInclusive<u8> = 20..=100;
    pub const FONT_SIZE: std::ops::RangeInclusive<u8> = 12..=20;

    fn validate(&self) -> SidekickResult<()> {
        if !Self::OPACITY.contains(&self.opacity) || !Self::FONT_SIZE.contains(&self.font_size) {
            return Err(SidekickError::new(
                ErrorCode::Internal,
                "Opacity must be 20 to 100% and text size 12 to 20 px.",
            ));
        }
        if self.positions.len() > sidekick_window::placement::MAX_SAVED {
            return Err(SidekickError::internal("Too many saved overlay positions."));
        }
        Ok(())
    }
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
    pub profile: Profile,
    pub overlay: OverlayPrefs,
    /// Product names, acronyms and people to boost in speech-to-text (PRD LIS-5).
    pub vocabulary: Vec<String>,
}

/// Most vocabulary terms, and the longest term.
pub const MAX_VOCABULARY: usize = 100;
pub const MAX_TERM: usize = 50;

/// Vocabulary terms trimmed, without blanks or duplicates (case-insensitive).
#[must_use]
pub fn clean_vocabulary(terms: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for t in terms {
        let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
        if !t.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&t)) {
            out.push(t);
        }
    }
    out
}

impl Settings {
    /// Speech-to-text keyterms: the user's names first (PRD LIS-5a), then the
    /// custom vocabulary (LIS-5), without duplicates.
    #[must_use]
    pub fn keyterms(&self) -> Vec<String> {
        let mut out = self.profile.names();
        for t in &self.vocabulary {
            if !out.iter().any(|o| o.eq_ignore_ascii_case(t)) {
                out.push(t.clone());
            }
        }
        out
    }
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
            profile: Profile::default(),
            overlay: OverlayPrefs::default(),
            vocabulary: Vec::new(),
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
    #[serde(default)]
    pub profile: Option<Profile>,
    #[serde(default)]
    pub overlay: Option<OverlayPrefs>,
    /// The full vocabulary list (PRD LIS-5).
    #[serde(default)]
    pub vocabulary: Option<Vec<String>>,
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
        if let Some(v) = patch.profile {
            v.validate()?;
            self.profile = Profile {
                name: v.name.trim().to_owned(),
                nicknames: v
                    .names()
                    .into_iter()
                    .skip(usize::from(!v.name.trim().is_empty()))
                    .collect(),
            };
        }
        if let Some(v) = patch.overlay {
            v.validate()?;
            self.overlay = v;
        }
        if let Some(v) = patch.vocabulary {
            let v = clean_vocabulary(&v);
            if v.len() > MAX_VOCABULARY || v.iter().any(|t| t.chars().count() > MAX_TERM) {
                return Err(SidekickError::new(
                    ErrorCode::Internal,
                    format!(
                        "Custom vocabulary can have up to {MAX_VOCABULARY} terms of up to {MAX_TERM} characters."
                    ),
                ));
            }
            self.vocabulary = v;
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
        profile: section(db, "profile")?.unwrap_or(d.profile),
        overlay: section::<OverlayPrefs>(db, "overlay")?
            .filter(|o| o.validate().is_ok())
            .unwrap_or(d.overlay),
        vocabulary: section(db, "vocabulary")?.unwrap_or(d.vocabulary),
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
    if patch.profile.is_some() {
        raw::set(db, "profile", &to_json(&updated.profile)?)?;
    }
    if patch.overlay.is_some() {
        raw::set(db, "overlay", &to_json(&updated.overlay)?)?;
    }
    if patch.vocabulary.is_some() {
        raw::set(db, "vocabulary", &to_json(&updated.vocabulary)?)?;
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
    fn profile_names_are_cleaned_and_persisted() {
        let db = db();
        assert!(load(&db).unwrap().profile.names().is_empty());
        let patch = SettingsPatch {
            profile: Some(Profile {
                name: "  Nitesh ".into(),
                nicknames: vec!["Nitu".into(), " ".into(), "nitesh".into(), "NK".into()],
            }),
            ..SettingsPatch::default()
        };
        let p = update(&db, patch).unwrap().profile;
        assert_eq!(p.name, "Nitesh");
        assert_eq!(
            p.nicknames,
            ["Nitu", "NK"],
            "blank and duplicate of the name removed"
        );
        assert_eq!(p.names(), ["Nitesh", "Nitu", "NK"]);
        assert_eq!(load(&db).unwrap().profile, p);
    }

    #[test]
    fn profile_limits() {
        let too_many = Profile {
            name: "A".into(),
            nicknames: (0..11).map(|i| format!("n{i}")).collect(),
        };
        assert!(
            update(
                &db(),
                SettingsPatch {
                    profile: Some(too_many),
                    ..SettingsPatch::default()
                }
            )
            .is_err()
        );
        let too_long = Profile {
            name: "x".repeat(41),
            nicknames: vec![],
        };
        assert!(
            update(
                &db(),
                SettingsPatch {
                    profile: Some(too_long),
                    ..SettingsPatch::default()
                }
            )
            .is_err()
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

    #[test]
    fn overlay_prefs_validate_and_persist() {
        let db = db();
        assert_eq!(load(&db).unwrap().overlay, OverlayPrefs::default());
        let prefs = OverlayPrefs {
            opacity: 60,
            font_size: 18,
            compact: true,
            positions: sidekick_window::placement::remember(
                &[],
                "Built-in@2880x1800",
                sidekick_window::placement::Rect {
                    x: 10,
                    y: 20,
                    width: 400,
                    height: 600,
                },
            ),
        };
        let patch = |o: OverlayPrefs| SettingsPatch {
            overlay: Some(o),
            ..SettingsPatch::default()
        };
        assert_eq!(update(&db, patch(prefs.clone())).unwrap().overlay, prefs);
        assert_eq!(load(&db).unwrap().overlay, prefs);
        for bad in [
            OverlayPrefs {
                opacity: 10,
                ..prefs.clone()
            },
            OverlayPrefs {
                opacity: 101,
                ..prefs.clone()
            },
            OverlayPrefs {
                font_size: 30,
                ..prefs.clone()
            },
        ] {
            assert!(update(&db, patch(bad)).is_err());
        }
        assert_eq!(
            load(&db).unwrap().overlay,
            prefs,
            "invalid patches change nothing"
        );
        // A stored section that no longer validates falls back to defaults.
        raw::set(
            &db,
            "overlay",
            &serde_json::json!({ "opacity": 5, "fontSize": 14, "compact": false, "positions": [] }),
        )
        .unwrap();
        assert_eq!(load(&db).unwrap().overlay, OverlayPrefs::default());
    }

    #[test]
    fn vocabulary_is_cleaned_limited_and_merged_after_the_names() {
        let db = db();
        let patch = |v: serde_json::Value| -> SettingsPatch { serde_json::from_value(v).unwrap() };
        let s = update(
            &db,
            patch(serde_json::json!({ "vocabulary": ["  Kubernetes ", "SOC 2", "kubernetes", "", "Priya   Raman"] })),
        )
        .unwrap();
        assert_eq!(s.vocabulary, ["Kubernetes", "SOC 2", "Priya Raman"]);
        assert_eq!(load(&db).unwrap().vocabulary, s.vocabulary, "saved");

        let too_many: Vec<String> = (0..=MAX_VOCABULARY).map(|i| format!("term {i}")).collect();
        assert!(update(&db, patch(serde_json::json!({ "vocabulary": too_many }))).is_err());
        assert!(
            update(
                &db,
                patch(serde_json::json!({ "vocabulary": ["x".repeat(51)] }))
            )
            .is_err()
        );

        let s = update(
            &db,
            patch(serde_json::json!({ "profile": { "name": "Priya Raman", "nicknames": ["PR"] } })),
        )
        .unwrap();
        assert_eq!(
            s.keyterms(),
            ["Priya Raman", "PR", "Kubernetes", "SOC 2"],
            "names first, no duplicates"
        );
    }
}
