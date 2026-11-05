//! LLM adapters. Each implements `sidekick_core::llm::LlmProvider` (tech doc 7.5).

pub mod anthropic;
pub mod http;
pub mod mock;
pub mod mock_embed;
pub mod openai;
