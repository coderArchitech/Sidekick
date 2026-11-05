//! Deterministic provider for tests and `SIDEKICK_MOCK_PROVIDERS=1` (tech doc 6.4).

use sidekick_core::SidekickResult;
use sidekick_core::llm::{
    FinishReason, LlmProvider, LlmRequest, ModelInfo, Role, StreamEvent, TokenStream, Usage,
};
use sidekick_secrets::SecretString;

use super::anthropic::async_stream;

#[derive(Debug, Clone, Default)]
pub struct MockProvider;

/// The text the mock answers with, for assertions.
#[must_use]
pub fn mock_answer(req: &LlmRequest) -> String {
    let last = req.messages.iter().rev().find(|m| m.role == Role::User);
    let question = last.map_or("", |m| m.content.as_str());
    if last.is_some_and(|m| m.image.is_some()) {
        format!("Mock answer (with screenshot) to: {question}")
    } else {
        format!("Mock answer to: {question}")
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
