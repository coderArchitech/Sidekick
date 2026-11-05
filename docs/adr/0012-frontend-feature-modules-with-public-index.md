# ADR-012: Frontend feature modules with a public index, enforced by lint

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Context

The frontend has around ten features (overlay, answers, chat, transcript, notes, library, settings, skills, orchestrator). Without rules, components import each other's internals and the IPC layer leaks into UI code, which makes features hard to change or remove.

## Decision

- Each feature lives in `src/features/<name>/` and exposes its public API through `index.ts`. Other code imports `@/features/<name>`, never a file inside it.
- Feature state lives inside the feature (`src/features/<name>/store.ts`). `src/stores/` is only for state shared by several features.
- All Rust calls go through `@/ipc`, which exposes the generated typed commands and swaps in mocks outside Tauri. Nothing else imports `@tauri-apps/api/core`.
- These rules are enforced by ESLint `no-restricted-imports` in `eslint.config.js`, so CI fails on a violation.

## Alternatives considered

- **Convention only.** Cheaper, but drifts quickly.
- **eslint-plugin-boundaries.** More expressive, but another dependency; `no-restricted-imports` covers the rules we need today.

## Consequences

- A feature can be rewritten freely as long as its `index.ts` API holds.
- UI can run in a plain browser (`pnpm dev:web`) and in tests with no Tauri runtime.
- The relative-path rule is coarse: deep imports between sibling features using `../other/file` are not caught. Use the `@/features/<name>` alias for all cross-feature imports.
