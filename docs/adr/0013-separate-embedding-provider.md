# ADR-013: Embeddings behind a separate EmbeddingProvider

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Context

The first draft put `embed()` on `LlmProvider` and fixed the vector table at 1024 dimensions. Anthropic, the likely default LLM, has no embeddings API, and different embedding models produce vectors of different sizes (OpenAI `text-embedding-3-small`: 1536, Voyage `voyage-3`: 1024, Ollama `nomic-embed-text`: 768).

## Decision

- A separate `EmbeddingProvider` trait (`id`, `model`, `dims`, `embed`) with adapters for OpenAI, Gemini, Voyage AI and Ollama.
- Default: OpenAI `text-embedding-3-small` (1536 dimensions). The user picks the embeddings provider in Settings, independent of the LLM.
- The `chunk_vectors` table is created by the store on first use with the active model's dimension, which is recorded in settings. Changing model drops and recreates it, and the re-embedding job refills it.

## Alternatives considered

- **Keep `embed()` on `LlmProvider`.** Forces a fake implementation for Anthropic and couples two independent choices.
- **Local-only embeddings.** Private and free, but adds a model download and CPU load to every install.
- **Fixed 1024-dimension table.** Rules out the most common default model without a migration.

## Consequences

- Users with only an Anthropic key need a second key (OpenAI, Gemini or Voyage) or Ollama for document search. Settings must make this clear.
- Switching embedding model costs a re-embed of all documents.
