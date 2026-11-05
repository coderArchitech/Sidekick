# ADR-003: Orchestration and prompts in TypeScript; OS, storage and network in Rust

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Decision

Orchestration and prompts in TypeScript; OS, storage and network in Rust.

## Alternatives considered

All logic in Rust.

## Rationale and trade-offs

Faster iteration and lower AI-assisted coding cost. Trade-off: two languages and an IPC boundary to maintain
