# ADR-005: Deepgram Nova-3 as default STT, behind a provider trait

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Decision

Deepgram Nova-3 as default STT, behind a provider trait.

## Alternatives considered

Single vendor; local-only Whisper.

## Rationale and trade-offs

Strong streaming latency and free credits; trait lets the user switch. Trade-off: adapters to maintain
