# ADR-004: SQLite + sqlite-vec + SQLCipher for all local data

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Decision

SQLite + sqlite-vec + SQLCipher for all local data.

## Alternatives considered

LanceDB, Chroma, separate files.

## Rationale and trade-offs

One encrypted file, simple backup, mature tooling. Trade-off: brute-force vector search, fine below about 100k chunks
