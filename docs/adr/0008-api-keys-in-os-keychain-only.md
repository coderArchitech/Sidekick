# ADR-008: API keys in OS keychain only

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Decision

API keys in OS keychain only.

## Alternatives considered

Encrypted file in app data.

## Rationale and trade-offs

OS-grade protection, no key management code. Trade-off: keychain prompts on some systems
