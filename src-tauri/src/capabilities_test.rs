//! Checks the window permission rules from tech doc 5.8 against capabilities/*.json.

use std::collections::HashMap;

fn permissions() -> HashMap<String, Vec<String>> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut out = HashMap::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(entry.unwrap().path()).unwrap()).unwrap();
        let windows: Vec<String> = serde_json::from_value(json["windows"].clone()).unwrap();
        let perms: Vec<String> = json["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str().map(str::to_owned))
            .collect();
        for w in windows {
            out.entry(w).or_insert_with(Vec::new).extend(perms.clone());
        }
    }
    out
}

/// Commands only the Settings window may call.
const SETTINGS_ONLY: &[&str] = &["allow-get-settings", "allow-update-settings"];

#[test]
fn settings_commands_are_granted_to_the_settings_window_only() {
    let perms = permissions();
    for p in SETTINGS_ONLY {
        assert!(
            perms["settings"].iter().any(|x| x == p),
            "settings window lacks {p}"
        );
        for (window, granted) in &perms {
            if window != "settings" {
                assert!(
                    !granted.iter().any(|x| x == p),
                    "{window} must not have {p}"
                );
            }
        }
    }
}

#[test]
fn every_declared_command_is_granted_somewhere() {
    let build_rs = include_str!("../build.rs");
    let start = build_rs.find("&[\"").unwrap();
    let end = build_rs[start..].find("];").unwrap() + start;
    let declared: Vec<String> = build_rs[start + 1..end]
        .split(',')
        .map(|s| {
            s.trim()
                .trim_matches(|c| c == '[' || c == '"' || c == ']')
                .to_owned()
        })
        .filter(|s| !s.is_empty())
        .collect();
    let all: Vec<String> = permissions().into_values().flatten().collect();
    for cmd in declared {
        let perm = format!("allow-{}", cmd.replace('_', "-"));
        assert!(
            all.contains(&perm),
            "{cmd} is declared in build.rs but no window is granted {perm}"
        );
    }
}
