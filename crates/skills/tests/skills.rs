//! Skills: templates, golden cases, the Skills folder and import/export (Task T-028).

#![allow(clippy::unwrap_used)]

use secrecy::SecretString;
use sidekick_skills::format::{self, Tool};
use sidekick_skills::repo::{self, TEMPLATES};
use sidekick_store::Database;

fn db() -> Database {
    Database::open_in_memory(&SecretString::from("s".repeat(64))).unwrap()
}

#[test]
fn every_template_validates_and_matches_its_file_name() {
    assert_eq!(TEMPLATES.len(), 6);
    for (id, text) in TEMPLATES {
        let s = format::validate(text).unwrap_or_else(|e| panic!("{id}: {e:?}"));
        assert_eq!(s.id, *id);
        assert!(!s.instructions.is_empty());
        assert!(
            s.tools.iter().all(|t| *t != Tool::WebSearch),
            "{id}: no web search until T-046"
        );
    }
}

#[test]
fn every_template_has_one_golden_answer_case() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../skills/golden");
    for (id, _) in TEMPLATES {
        let text = std::fs::read_to_string(dir.join(format!("{id}.json")))
            .unwrap_or_else(|_| panic!("missing skills/golden/{id}.json"));
        let g: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(g["skill"], *id);
        assert!(g["question"].as_str().is_some_and(|q| !q.is_empty()));
        assert!(!g["documents"].as_array().unwrap().is_empty());
        assert!(!g["mustMention"].as_array().unwrap().is_empty());
    }
    let cases = std::fs::read_dir(&dir).unwrap().count();
    assert_eq!(cases, TEMPLATES.len(), "no orphan golden cases");
}

#[test]
fn seeds_templates_once_then_lists_by_name() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(repo::seed(&db, dir.path()).unwrap(), 6);
    assert_eq!(repo::seed(&db, dir.path()).unwrap(), 0, "only on first run");
    let names: Vec<String> = repo::list(&db)
        .unwrap()
        .into_iter()
        .map(|s| s.skill.name)
        .collect();
    assert_eq!(
        names,
        [
            "Interview Coach",
            "Negotiation Coach",
            "Sales Discovery",
            "Staff Engineer Interview (JS)",
            "Teacher",
            "Technical Architect"
        ]
    );
    assert!(
        dir.path().join("teacher.md").exists(),
        "files are the source of truth"
    );
    assert!(
        repo::list(&db)
            .unwrap()
            .iter()
            .all(|s| s.builtin && s.enabled)
    );
}

#[test]
fn save_edit_disable_and_delete() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    let mut skill = format::parse(TEMPLATES[0].1).unwrap();
    skill.id = "my-coach".into();
    skill.name = "My Coach".into();
    let saved = repo::save(&db, dir.path(), &skill).unwrap();
    assert!(!saved.builtin);
    skill.instructions = "Shorter answers.".into();
    assert_eq!(
        repo::save(&db, dir.path(), &skill)
            .unwrap()
            .skill
            .instructions,
        "Shorter answers."
    );
    assert!(!repo::set_enabled(&db, "my-coach", false).unwrap().enabled);
    // Invalid Skills are never written.
    let mut bad = skill.clone();
    bad.id = "Not Valid".into();
    assert!(repo::save(&db, dir.path(), &bad).is_err());
    assert!(!dir.path().join("Not Valid.md").exists());

    repo::delete(&db, dir.path(), "my-coach").unwrap();
    assert!(repo::get(&db, "my-coach").is_err());
    assert!(!dir.path().join("my-coach.md").exists());
    repo::delete(&db, dir.path(), "my-coach").unwrap(); // idempotent
}

#[test]
fn import_lists_tools_for_approval_without_saving() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("shared.md");
    std::fs::write(
        &file,
        "---\nschema_version: 1\nid: shared\nname: Shared\ntools: [web_search, calculator]\n---\nUse the web.\n",
    )
    .unwrap();
    let p = repo::preview_import(&db, &file).unwrap();
    assert_eq!(p.requested_tools, vec![Tool::WebSearch, Tool::Calculator]);
    assert!(!p.replaces_existing);
    assert!(repo::list(&db).unwrap().is_empty(), "nothing saved yet");

    repo::save(&db, &dir.path().join("skills"), &p.skill).unwrap();
    assert!(repo::preview_import(&db, &file).unwrap().replaces_existing);
}

#[test]
fn import_rejects_unknown_tools_and_huge_files() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("evil.md");
    std::fs::write(
        &file,
        "---\nschema_version: 1\nid: evil\nname: Evil\ntools: [shell]\n---\nrm -rf\n",
    )
    .unwrap();
    let err = repo::preview_import(&db, &file).unwrap_err();
    assert!(
        err.message.contains("tools[0]") && err.message.contains("isn't an allowed tool"),
        "{}",
        err.message
    );

    let big = dir.path().join("big.md");
    std::fs::write(&big, "x".repeat(70_000)).unwrap();
    assert!(
        repo::preview_import(&db, &big)
            .unwrap_err()
            .message
            .contains("too large")
    );
}

#[test]
fn export_then_import_is_lossless() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    repo::seed(&db, &dir.path().join("skills")).unwrap();
    for (id, _) in TEMPLATES {
        let out = dir.path().join(format!("export-{id}.md"));
        repo::export(&db, id, &out).unwrap();
        let back = repo::preview_import(&db, &out).unwrap().skill;
        assert_eq!(back, repo::get(&db, id).unwrap().skill, "{id}");
    }
}

#[test]
fn an_update_adds_new_built_in_skills_once_without_restoring_deleted_ones() {
    let db = db();
    let dir = tempfile::tempdir().unwrap();
    // An install from before the interview Skill: the first five were seeded,
    // and the user deleted one of them.
    for (id, text) in TEMPLATES
        .iter()
        .filter(|(id, _)| *id != "staff-engineer-interview")
    {
        repo::save(
            &db,
            dir.path(),
            &sidekick_skills::format::parse(text).unwrap(),
        )
        .unwrap();
        let _ = id;
    }
    repo::delete(&db, dir.path(), "teacher").unwrap();

    assert_eq!(repo::seed(&db, dir.path()).unwrap(), 1, "only the new one");
    assert!(repo::get(&db, "staff-engineer-interview").is_ok());
    assert!(
        repo::get(&db, "teacher").is_err(),
        "a deleted built-in stays deleted"
    );

    // Offered once: deleting it later doesn't bring it back.
    repo::delete(&db, dir.path(), "staff-engineer-interview").unwrap();
    assert_eq!(repo::seed(&db, dir.path()).unwrap(), 0);
    assert!(repo::get(&db, "staff-engineer-interview").is_err());
}

#[test]
fn the_interview_skill_covers_js_coding_and_system_design() {
    let (_, text) = TEMPLATES
        .iter()
        .find(|(id, _)| *id == "staff-engineer-interview")
        .unwrap();
    let skill = sidekick_skills::format::parse(text).unwrap();
    assert_eq!(skill.name, "Staff Engineer Interview (JS)");
    for must in [
        "Staff Engineer",
        "JavaScript",
        "Coding round",
        "Low-level design",
        "High-level design (HLD), backend",
        "High-level design (HLD), frontend",
        "first person",
    ] {
        assert!(skill.instructions.contains(must), "missing {must}");
    }
    assert!(skill.instructions.chars().count() <= 8_000);
}
