# ADR-011: Cargo workspace with one crate per Rust module

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Context

The tech doc (section 3.2) splits the Rust core into 15 modules (`audio`, `stt`, `store`, `gateway`, ...). Inside a single crate, nothing stops one module reaching into another's internals, and every change recompiles the whole core. The owner asked for a codebase that is modular, clearly separated and easy to change.

## Decision

The Rust core is a Cargo workspace:

- `crates/<module>/` holds one crate per module, named `sidekick-<module>` (for example `crates/store` is `sidekick-store`).
- `crates/core` (`sidekick-core`) holds only shared types: `SidekickError`, IDs, provider traits. It depends on no other Sidekick crate and not on Tauri.
- `src-tauri/` (`sidekick-app`) is a thin shell: it builds the Tauri app, registers IPC commands, and wires crates together. Commands validate input, call a crate, and map the result. No business logic lives there.
- Domain crates do not depend on Tauri. Anything that needs the app handle (emitting events, windows) receives a small trait or callback from the shell.
- Crates are created when their first code lands, not ahead of time.

## Alternatives considered

- **One crate with modules** (the original layout in section 7.1). Simpler to start, but boundaries are only by convention and builds are slower as the core grows.
- **One crate per provider** (for example `stt-deepgram`). Too fine-grained for a single developer; providers stay as files inside their crate.

## Consequences

- Boundaries are enforced by the compiler: a crate can only use what another crate makes `pub` and declares as a dependency.
- Most crates can be unit-tested without Tauri, a WebView or system libraries, which keeps tests fast.
- Swapping an implementation (for example the vector store) touches one crate plus the shell wiring.
- Cost: more `Cargo.toml` files, and shared dependency versions must live in `[workspace.dependencies]`.
