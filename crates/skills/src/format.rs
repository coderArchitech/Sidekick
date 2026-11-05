//! The Skill file format (tech doc 5.7).

use std::sync::LazyLock;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sidekick_core::{ErrorCode, SidekickError};

/// The JSON Schema for the front matter, shared with docs and tooling.
pub const SCHEMA: &str = include_str!("../../../schemas/skill.schema.json");
pub const SCHEMA_VERSION: u32 = 1;
/// Longest instructions body, so a Skill can't crowd out the meeting context.
pub const MAX_INSTRUCTIONS: usize = 8_000;

static VALIDATOR: LazyLock<Option<jsonschema::Validator>> = LazyLock::new(|| {
    let schema: Value = serde_json::from_str(SCHEMA).ok()?;
    jsonschema::validator_for(&schema).ok()
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Trigger {
    QuestionDetected,
    Chat,
    Screenshot,
}

/// The tool allowlist (tech doc 11.5). Tool use itself arrives with Agents (T-045).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum Tool {
    DocSearch,
    WebSearch,
    Calculator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum AnswerFormat {
    #[default]
    CueThenFull,
    FullOnly,
}

fn all_triggers() -> Vec<Trigger> {
    vec![
        Trigger::QuestionDetected,
        Trigger::Chat,
        Trigger::Screenshot,
    ]
}

/// A Skill: front matter fields plus the instructions body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct Skill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "all_triggers")]
    pub triggers: Vec<Trigger>,
    #[serde(default)]
    pub tools: Vec<Tool>,
    #[serde(default)]
    pub answer_format: AnswerFormat,
    #[serde(default)]
    pub knowledge: Vec<String>,
    pub instructions: String,
}

/// Front matter as stored in the file (snake_case keys, tech doc 5.7).
#[derive(Debug, Serialize, Deserialize)]
struct Front {
    schema_version: u32,
    id: String,
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    description: String,
    #[serde(default = "all_triggers")]
    triggers: Vec<Trigger>,
    #[serde(default)]
    tools: Vec<Tool>,
    #[serde(default)]
    answer_format: AnswerFormat,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    knowledge: Vec<String>,
}

/// One problem in a Skill file, for the builder and import dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "camelCase")]
pub struct FieldError {
    /// `tools[1]`, `name`, `instructions`, or empty for the whole file.
    pub field: String,
    pub message: String,
}

fn field_error(field: &str, message: impl Into<String>) -> FieldError {
    FieldError {
        field: field.to_owned(),
        message: message.into(),
    }
}

/// `/tools/1` → `tools[1]`.
fn field_name(pointer: &str) -> String {
    let mut out = String::new();
    for part in pointer.split('/').filter(|p| !p.is_empty()) {
        if part.chars().all(|c| c.is_ascii_digit()) {
            out.push_str(&format!("[{part}]"));
        } else {
            if !out.is_empty() {
                out.push('.');
            }
            out.push_str(part);
        }
    }
    out
}

/// Friendlier messages for the common cases; the schema's own otherwise.
fn describe(field: &str, instance: &Value, raw: String) -> String {
    if field.starts_with("tools") && instance.is_string() {
        return format!(
            "{instance} isn't an allowed tool. Allowed: doc_search, web_search, calculator."
        );
    }
    if field.starts_with("triggers") && instance.is_string() {
        return format!("{instance} isn't a trigger. Use question_detected, chat or screenshot.");
    }
    if field == "id" {
        return "Use lowercase letters, digits and dashes (up to 64).".into();
    }
    raw
}

fn split_front_matter(text: &str) -> Result<(&str, &str), FieldError> {
    let text = text.trim_start_matches('\u{feff}');
    let rest = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
        .ok_or_else(|| {
            field_error(
                "",
                "A Skill file starts with front matter between --- lines.",
            )
        })?;
    let end = rest
        .find("\n---")
        .ok_or_else(|| field_error("", "The front matter isn't closed with a --- line."))?;
    let front = &rest[..end];
    let body = rest[end + 4..].trim_start_matches(['\r', '\n']);
    Ok((front, body))
}

/// Parses and validates a Skill file. Every problem is reported, by field.
pub fn validate(text: &str) -> Result<Skill, Vec<FieldError>> {
    let (front, body) = split_front_matter(text).map_err(|e| vec![e])?;
    let value: Value = serde_yaml_ng::from_str(front).map_err(|e| {
        vec![field_error(
            "",
            format!("The front matter isn't valid YAML: {e}"),
        )]
    })?;
    let Some(validator) = VALIDATOR.as_ref() else {
        return Err(vec![field_error(
            "",
            "The Skill schema couldn't be loaded.",
        )]);
    };
    let mut errors: Vec<FieldError> = validator
        .iter_errors(&value)
        .map(|e| {
            let field = field_name(&e.instance_path().to_string());
            let message = describe(&field, e.instance(), e.to_string());
            FieldError { field, message }
        })
        .collect();
    let instructions = body.trim().to_owned();
    if instructions.is_empty() {
        errors.push(field_error(
            "instructions",
            "Write the instructions below the front matter.",
        ));
    } else if instructions.chars().count() > MAX_INSTRUCTIONS {
        errors.push(field_error(
            "instructions",
            format!("Keep the instructions under {MAX_INSTRUCTIONS} characters."),
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let front: Front = serde_json::from_value(value)
        .map_err(|e| vec![field_error("", format!("Unexpected front matter: {e}"))])?;
    Ok(Skill {
        id: front.id,
        name: front.name.trim().to_owned(),
        description: front.description.trim().to_owned(),
        triggers: front.triggers,
        tools: front.tools,
        answer_format: front.answer_format,
        knowledge: front.knowledge,
        instructions,
    })
}

/// Joins field errors into one user-facing error.
#[must_use]
pub fn to_error(errors: &[FieldError]) -> SidekickError {
    let text = errors
        .iter()
        .map(|e| {
            if e.field.is_empty() {
                e.message.clone()
            } else {
                format!("{}: {}", e.field, e.message)
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    SidekickError::new(
        ErrorCode::ParseFailed,
        format!("This Skill file has problems. {text}"),
    )
}

/// Parses a Skill file into a `SidekickError` on failure.
pub fn parse(text: &str) -> Result<Skill, SidekickError> {
    validate(text).map_err(|e| to_error(&e))
}

/// Writes a Skill file. `validate(to_markdown(s))` gives `s` back.
#[must_use]
pub fn to_markdown(skill: &Skill) -> String {
    let front = Front {
        schema_version: SCHEMA_VERSION,
        id: skill.id.clone(),
        name: skill.name.clone(),
        description: skill.description.clone(),
        triggers: skill.triggers.clone(),
        tools: skill.tools.clone(),
        answer_format: skill.answer_format,
        knowledge: skill.knowledge.clone(),
    };
    let yaml = serde_yaml_ng::to_string(&front).unwrap_or_default();
    format!("---\n{}---\n{}\n", yaml, skill.instructions.trim())
}

/// A file-name-safe id from a name: "Sales Discovery!" → "sales-discovery".
#[must_use]
pub fn slug(name: &str) -> String {
    let mut out = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else if !out.ends_with('-') && !out.is_empty() {
            out.push('-');
        }
    }
    let out = out.trim_end_matches('-');
    let out: String = out.chars().take(64).collect();
    if out.is_empty() { "skill".into() } else { out }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const VALID: &str =
        "---\nschema_version: 1\nid: demo\nname: Demo\ntools: [doc_search]\n---\nBe helpful.\n";

    #[test]
    fn parses_a_minimal_skill_with_defaults() {
        let s = parse(VALID).unwrap();
        assert_eq!(s.id, "demo");
        assert_eq!(s.tools, vec![Tool::DocSearch]);
        assert_eq!(s.triggers.len(), 3, "all triggers by default");
        assert_eq!(s.answer_format, AnswerFormat::CueThenFull);
        assert_eq!(s.instructions, "Be helpful.");
    }

    #[test]
    fn reports_every_problem_by_field() {
        let bad = "---\nschema_version: 2\nid: Bad Id\nname: ''\ntools: [doc_search, shell]\ntriggers: [always]\nextra: 1\n---\n";
        let errs = validate(bad).unwrap_err();
        let fields: Vec<&str> = errs.iter().map(|e| e.field.as_str()).collect();
        for f in [
            "schema_version",
            "id",
            "name",
            "tools[1]",
            "triggers[0]",
            "instructions",
        ] {
            assert!(fields.contains(&f), "missing {f} in {errs:?}");
        }
        let tool = errs.iter().find(|e| e.field == "tools[1]").unwrap();
        assert!(
            tool.message.contains("\"shell\" isn't an allowed tool"),
            "{}",
            tool.message
        );
        assert!(
            errs.iter().any(|e| e.message.contains("extra")),
            "unknown keys rejected: {errs:?}"
        );
    }

    #[test]
    fn rejects_files_without_front_matter_or_valid_yaml() {
        assert_eq!(validate("Just text").unwrap_err()[0].field, "");
        assert!(
            validate("---\nname: [unclosed\n---\nx").unwrap_err()[0]
                .message
                .contains("YAML")
        );
        assert!(
            validate("---\nid: x\n").unwrap_err()[0]
                .message
                .contains("closed")
        );
    }

    #[test]
    fn round_trips_through_markdown() {
        let s = Skill {
            id: "round-trip".into(),
            name: "Round: Trip \"quoted\"".into(),
            description: "Has: colons, # hashes and 'quotes'".into(),
            triggers: vec![Trigger::Chat],
            tools: vec![Tool::Calculator, Tool::WebSearch],
            answer_format: AnswerFormat::FullOnly,
            knowledge: vec!["spec v2.pdf".into()],
            instructions: "Line one.\n\n---\n\nA rule above stays in the body.".into(),
        };
        assert_eq!(parse(&to_markdown(&s)).unwrap(), s);
    }

    #[test]
    fn slugs_names() {
        assert_eq!(slug("Sales Discovery!"), "sales-discovery");
        assert_eq!(slug("  ***  "), "skill");
        assert_eq!(slug(&"a".repeat(100)).len(), 64);
    }
}
