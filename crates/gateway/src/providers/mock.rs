//! Deterministic provider for tests and `SIDEKICK_MOCK_PROVIDERS=1` (tech doc 6.4).

use sidekick_core::SidekickResult;
use sidekick_core::llm::{
    FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_secrets::SecretString;

use super::anthropic::async_stream;

#[derive(Debug, Clone, Default)]
pub struct MockProvider;

/// The start of the agent prompt (frontend `orchestrator/agent.ts`).
const AGENT_PROMPT: &str = "You decide whether tools are needed";

/// The longest run of arithmetic in `text` with an operator between digits,
/// e.g. "2400 * 15%" in "What is 2400 * 15% of the deal?".
fn arithmetic(text: &str) -> Option<String> {
    let allowed = |c: char| c.is_ascii_digit() || " .+-*/^()%".contains(c);
    let mut best: Option<String> = None;
    for run in text.split(|c: char| !allowed(c)) {
        let run = run.trim();
        let digits = run.chars().filter(char::is_ascii_digit).count();
        let ops = run.chars().filter(|c| "+-*/^".contains(*c)).count();
        if digits >= 2 && ops >= 1 && best.as_ref().is_none_or(|b| run.len() > b.len()) {
            best = Some(run.to_owned());
        }
    }
    best
}

/// The question inside the agent prompt's `<question>` tags.
fn question_of(content: &str) -> &str {
    content
        .split_once("<question>")
        .and_then(|(_, rest)| rest.split_once("</question>"))
        .map_or(content, |(q, _)| q.trim())
}

/// The text the mock answers with, for assertions. For the agent's tool
/// step (PRD SKL-4) it is deterministic, once per question: the calculator
/// for arithmetic, web search for "today", "latest", "current" or "now"
/// (when the Skill allows them); otherwise done.
#[must_use]
pub fn mock_answer(req: &LlmRequest) -> String {
    let last = req.messages.iter().rev().find(|m| m.role == Role::User);
    if req
        .system
        .as_deref()
        .is_some_and(|s| s.starts_with(AGENT_PROMPT))
    {
        let content = last.map_or("", |m| m.content.as_str());
        let system = req.system.as_deref().unwrap_or_default();
        if !content.contains("<tool_results>") {
            let question = question_of(content);
            if system.contains("- calculator:")
                && let Some(expr) = arithmetic(question)
            {
                return format!("{{\"tool\": \"calculator\", \"input\": \"{expr}\"}}");
            }
            let lower = question.to_lowercase();
            let current = ["today", "latest", "current", "now"]
                .iter()
                .any(|w| lower.split(|c: char| !c.is_alphanumeric()).any(|x| x == *w));
            if system.contains("- web_search:") && current {
                let q = question.replace('"', "'");
                return format!("{{\"tool\": \"web_search\", \"input\": \"{q}\"}}");
            }
        }
        return "{\"done\": true}".to_owned();
    }
    let question = last.map_or("", |m| m.content.as_str());
    let answer = if last.is_some_and(|m| m.image.is_some()) {
        format!("Mock answer (with screenshot) to: {question}")
    } else {
        format!("Mock answer to: {question}")
    };
    // Full answers (answer-sized budgets, not one-line cues) cite the first
    // source, so E2E tests see citations as with a real model.
    if req.max_tokens > 100 {
        format!("{answer} [1]")
    } else {
        answer
    }
}

#[async_trait::async_trait]
impl LlmProvider for MockProvider {
    fn id(&self) -> &'static str {
        "mock"
    }

    async fn list_models(&self, _key: Option<&SecretString>) -> SidekickResult<Vec<ModelInfo>> {
        Ok(vec![
            ModelInfo {
                id: "mock-fast".into(),
                display_name: "Mock (fast)".into(),
            },
            ModelInfo {
                id: "mock-large".into(),
                display_name: "Mock (large)".into(),
            },
        ])
    }

    async fn stream(
        &self,
        req: LlmRequest,
        _key: Option<&SecretString>,
    ) -> SidekickResult<TokenStream> {
        // E2E failure tests (NFR-5): every answer fails like an outage.
        if std::env::var("SIDEKICK_MOCK_LLM_FAIL").as_deref() == Ok("1") {
            return Err(sidekick_core::SidekickError::new(
                sidekick_core::ErrorCode::ProviderUnavailable,
                "The mock provider is down (SIDEKICK_MOCK_LLM_FAIL).",
            ));
        }
        let text = mock_answer(&req);
        Ok(async_stream(move |tx| async move {
            let words: Vec<String> = text.split_inclusive(' ').map(str::to_owned).collect();
            let output_tokens = u32::try_from(words.len()).unwrap_or(u32::MAX);
            for w in words {
                if tx.send(Ok(StreamEvent::Delta(w))).await.is_err() {
                    return;
                }
                tokio::task::yield_now().await;
            }
            let _ = tx
                .send(Ok(StreamEvent::Done {
                    usage: Usage {
                        input_tokens: 10,
                        output_tokens,
                    },
                    finish_reason: FinishReason::EndTurn,
                }))
                .await;
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidekick_core::llm::ChatMessage;

    fn req(system: &str, content: &str) -> LlmRequest {
        LlmRequest {
            model: "mock-fast".into(),
            system: Some(system.into()),
            messages: vec![ChatMessage {
                role: Role::User,
                content: content.into(),
                screenshot_id: None,
                image: None,
            }],
            max_tokens: 200,
            effort: None,
        }
    }

    #[test]
    fn agent_step_uses_the_calculator_once_for_arithmetic() {
        let agent = format!(
            "{AGENT_PROMPT} before a meeting question is answered.\n- calculator: math\n- web_search: web"
        );
        assert_eq!(
            mock_answer(&req(
                &agent,
                "<question>\nWhat is 2400 * 15% of the deal?\n</question>"
            )),
            r#"{"tool": "calculator", "input": "2400 * 15%"}"#
        );
        assert_eq!(
            mock_answer(&req(
                &agent,
                "<question>2400 * 15%</question>\n<tool_results>\n- x\n</tool_results>"
            )),
            r#"{"done": true}"#
        );
        assert_eq!(
            mock_answer(&req(&agent, "<question>What's the Pro limit?</question>")),
            r#"{"done": true}"#
        );
        assert_eq!(
            mock_answer(&req(
                &agent,
                "<question>What is the latest Fed rate?</question>"
            )),
            r#"{"tool": "web_search", "input": "What is the latest Fed rate?"}"#
        );
        // Only tools the Skill allows.
        let calc_only = format!("{AGENT_PROMPT}.\n- calculator: math");
        assert_eq!(
            mock_answer(&req(
                &calc_only,
                "<question>What is the latest Fed rate?</question>"
            )),
            r#"{"done": true}"#
        );
        // Other prompts still echo.
        assert!(mock_answer(&req("Something else", "2 + 2")).starts_with("Mock answer to:"));
    }

    #[test]
    fn finds_the_longest_arithmetic_run() {
        assert_eq!(
            arithmetic("Is (12 + 3) * 4 more than 50?").as_deref(),
            Some("(12 + 3) * 4")
        );
        assert_eq!(arithmetic("Room 101, floor 3"), None);
        assert_eq!(arithmetic("2024-11-08"), Some("2024-11-08".into()));
    }
}
