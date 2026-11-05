# ADR-010: WebdriverIO for full-app E2E; Playwright for UI-only tests

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Decision

WebdriverIO for full-app E2E; Playwright for UI-only tests.

## Alternatives considered

Playwright only.

## Rationale and trade-offs

Only WebdriverIO drives a Tauri app on both OSes. Trade-off: two test runners
