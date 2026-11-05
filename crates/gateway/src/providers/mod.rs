//! LLM adapters. Each implements `sidekick_core::llm::LlmProvider` (tech doc 7.5).

pub mod anthropic;
pub mod gemini;
pub mod http;
pub mod mock;
pub mod mock_embed;
pub mod ollama;
pub mod openai;
pub mod openai_chat;
pub mod pump;
