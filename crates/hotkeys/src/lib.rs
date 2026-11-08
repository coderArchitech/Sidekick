//! Global shortcuts (PRD UI-6, LIS-6, QA-3; tech doc 12.3).
//!
//! Shortcuts are written in Tauri's accelerator syntax, e.g. `CmdOrCtrl+Shift+H`
//! (`CmdOrCtrl` is Cmd on macOS and Ctrl on Windows). Every shortcut needs at
//! least one modifier so a global shortcut never swallows normal typing.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

/// What a shortcut does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Action {
    ShowHide,
    Ask,
    Screenshot,
    AutoAnswer,
    PauseResume,
    SwitchSkill,
    Bookmark,
    /// Clicks pass through the overlay to the app behind (PRD UI-5).
    ClickThrough,
}

impl Action {
    pub const ALL: [Self; 8] = [
        Self::ShowHide,
        Self::Ask,
        Self::Screenshot,
        Self::AutoAnswer,
        Self::PauseResume,
        Self::SwitchSkill,
        Self::Bookmark,
        Self::ClickThrough,
    ];
}

/// One action and its key combination.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct Binding {
    pub action: Action,
    pub keys: String,
}

/// Defaults from tech doc 12.3.
#[must_use]
pub fn defaults() -> Vec<Binding> {
    [
        (Action::ShowHide, "CmdOrCtrl+Shift+H"),
        (Action::Ask, "CmdOrCtrl+Shift+Space"),
        (Action::Screenshot, "CmdOrCtrl+Shift+S"),
        (Action::AutoAnswer, "CmdOrCtrl+Shift+A"),
        (Action::PauseResume, "CmdOrCtrl+Shift+P"),
        (Action::SwitchSkill, "CmdOrCtrl+Shift+K"),
        (Action::Bookmark, "CmdOrCtrl+Shift+B"),
        (Action::ClickThrough, "CmdOrCtrl+Shift+T"),
    ]
    .into_iter()
    .map(|(action, keys)| Binding {
        action,
        keys: keys.into(),
    })
    .collect()
}

const MODIFIERS: [(&str, &str); 9] = [
    ("cmdorctrl", "CmdOrCtrl"),
    ("commandorcontrol", "CmdOrCtrl"),
    ("ctrl", "Ctrl"),
    ("control", "Ctrl"),
    ("alt", "Alt"),
    ("option", "Alt"),
    ("shift", "Shift"),
    ("cmd", "Super"),
    ("super", "Super"),
];

fn invalid(keys: &str, why: &str) -> SidekickError {
    SidekickError::new(
        ErrorCode::Internal,
        format!("\"{keys}\" isn't a valid shortcut: {why}."),
    )
}

/// Canonical form: modifiers in a fixed order, then the key, e.g. `CmdOrCtrl+Shift+H`.
pub fn normalize(keys: &str) -> SidekickResult<String> {
    let mut mods = BTreeSet::new();
    let mut key: Option<String> = None;
    for part in keys.split('+').map(str::trim) {
        if part.is_empty() {
            return Err(invalid(keys, "it has an empty part"));
        }
        if let Some((_, canonical)) = MODIFIERS
            .iter()
            .find(|(name, _)| part.eq_ignore_ascii_case(name))
        {
            mods.insert(order(canonical));
            continue;
        }
        if key.is_some() {
            return Err(invalid(keys, "it has more than one non-modifier key"));
        }
        key = Some(
            normalize_key(part)
                .ok_or_else(|| invalid(keys, "use a letter, digit, F1–F24 or Space"))?,
        );
    }
    let key = key.ok_or_else(|| invalid(keys, "it needs a key, not just modifiers"))?;
    if mods.is_empty() {
        return Err(invalid(
            keys,
            "it needs at least one modifier (Ctrl, Cmd, Alt or Shift)",
        ));
    }
    let mut parts: Vec<String> = mods.into_iter().map(|(_, m)| m.to_owned()).collect();
    parts.push(key);
    Ok(parts.join("+"))
}

fn order(m: &str) -> (u8, &str) {
    let rank = match m {
        "CmdOrCtrl" => 0,
        "Ctrl" => 1,
        "Super" => 2,
        "Alt" => 3,
        _ => 4, // Shift
    };
    (rank, m)
}

fn normalize_key(k: &str) -> Option<String> {
    let upper = k.to_ascii_uppercase();
    if upper.len() == 1 && upper.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Some(upper);
    }
    if upper == "SPACE" {
        return Some("Space".into());
    }
    if let Some(n) = upper.strip_prefix('F').and_then(|n| n.parse::<u8>().ok())
        && (1..=24).contains(&n)
    {
        return Some(format!("F{n}"));
    }
    None
}

/// Checks a full set: every action exactly once, every shortcut valid and unique.
/// Returns the bindings with normalised keys.
pub fn validate(bindings: &[Binding]) -> SidekickResult<Vec<Binding>> {
    let mut seen_actions = BTreeSet::new();
    let mut seen_keys: BTreeMap<String, Action> = BTreeMap::new();
    let mut out = Vec::with_capacity(bindings.len());
    for b in bindings {
        if !seen_actions.insert(b.action) {
            return Err(SidekickError::new(
                ErrorCode::Internal,
                format!("{:?} has more than one shortcut.", b.action),
            ));
        }
        let keys = normalize(&b.keys)?;
        if let Some(other) = seen_keys.insert(keys.clone(), b.action) {
            return Err(SidekickError::new(
                ErrorCode::Internal,
                format!(
                    "{keys} is used for both {} and {}.",
                    label(other),
                    label(b.action)
                ),
            ));
        }
        out.push(Binding {
            action: b.action,
            keys,
        });
    }
    if let Some(missing) = Action::ALL.iter().find(|a| !seen_actions.contains(a)) {
        return Err(SidekickError::new(
            ErrorCode::Internal,
            format!("{} has no shortcut.", label(*missing)),
        ));
    }
    Ok(out)
}

/// User-facing name of an action.
#[must_use]
pub const fn label(action: Action) -> &'static str {
    match action {
        Action::ShowHide => "Show / hide overlay",
        Action::Ask => "Ask (focus Chat)",
        Action::Screenshot => "Screenshot",
        Action::AutoAnswer => "Auto-answer on/off",
        Action::PauseResume => "Pause / resume listening",
        Action::SwitchSkill => "Switch Skill",
        Action::Bookmark => "Bookmark this moment",
        Action::ClickThrough => "Click-through on/off",
    }
}

/// Adds the default shortcut of any action missing from a saved set (one
/// added in a newer version), unless its keys are already taken. Then
/// `validate` decides as usual.
#[must_use]
pub fn complete(bindings: &[Binding]) -> Vec<Binding> {
    let mut out = bindings.to_vec();
    for d in defaults() {
        let missing = !out.iter().any(|b| b.action == d.action);
        let taken = out
            .iter()
            .any(|b| normalize(&b.keys).ok().as_deref() == Some(d.keys.as_str()));
        if missing && !taken {
            out.push(d);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_tech_doc_and_are_valid() {
        let d = defaults();
        assert_eq!(d.len(), Action::ALL.len());
        assert_eq!(
            d[0],
            Binding {
                action: Action::ShowHide,
                keys: "CmdOrCtrl+Shift+H".into()
            }
        );
        assert_eq!(validate(&d).unwrap(), d, "defaults are already normalised");
    }

    #[test]
    fn normalises_spelling_case_and_order() {
        assert_eq!(normalize("shift+cmdorctrl+h").unwrap(), "CmdOrCtrl+Shift+H");
        assert_eq!(normalize("Control + Alt + f5").unwrap(), "Ctrl+Alt+F5");
        assert_eq!(normalize("Cmd+Option+space").unwrap(), "Super+Alt+Space");
        assert_eq!(
            normalize("CommandOrControl+Shift+1").unwrap(),
            "CmdOrCtrl+Shift+1"
        );
    }

    #[test]
    fn rejects_invalid_shortcuts() {
        for bad in [
            "H",
            "Shift",
            "CmdOrCtrl+Shift+H+J",
            "Ctrl++H",
            "Ctrl+Enter",
            "Ctrl+F25",
            "",
        ] {
            assert!(normalize(bad).is_err(), "{bad:?} should be rejected");
        }
        assert!(normalize("H").unwrap_err().message.contains("modifier"));
    }

    #[test]
    fn rejects_duplicates_and_missing_actions() {
        let mut b = defaults();
        b[1].keys = "shift+CmdOrCtrl+h".into(); // same as ShowHide once normalised
        let err = validate(&b).unwrap_err();
        assert!(
            err.message.contains("Show / hide overlay") && err.message.contains("Ask"),
            "{}",
            err.message
        );

        let mut b = defaults();
        b.pop();
        assert!(validate(&b).unwrap_err().message.contains("Click-through"));

        let mut b = defaults();
        b.push(Binding {
            action: Action::Ask,
            keys: "Ctrl+Alt+Q".into(),
        });
        assert!(validate(&b).is_err());
    }

    #[test]
    fn serialises_actions_for_the_ui() {
        assert_eq!(
            serde_json::to_value(Action::PauseResume).unwrap(),
            "pause_resume"
        );
    }

    #[test]
    fn complete_adds_new_actions_without_touching_custom_keys() {
        let mut saved = defaults();
        saved.retain(|b| b.action != Action::ClickThrough);
        saved[0].keys = "CmdOrCtrl+Alt+O".into();
        let done = validate(&complete(&saved)).unwrap();
        assert_eq!(done[0].keys, "CmdOrCtrl+Alt+O", "custom key kept");
        assert_eq!(
            done.iter()
                .find(|b| b.action == Action::ClickThrough)
                .map(|b| b.keys.as_str()),
            Some("CmdOrCtrl+Shift+T")
        );
        // If the default keys are taken, nothing is added and validation fails.
        saved[0].keys = "CmdOrCtrl+Shift+T".into();
        assert!(validate(&complete(&saved)).is_err());
    }
}
