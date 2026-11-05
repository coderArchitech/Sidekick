# Sidekick

🦸 Your private, real-time AI meeting copilot. Sidekick listens to a live conversation in any meeting app (Teams, Zoom, Google Meet, Webex and others), keeps running notes, detects questions as they're asked (especially ones addressed to you by name), and suggests answers with sources from your documents, the meeting so far, screenshots and the web. Answers appear in a translucent overlay that only you can see.

> **Status:** early development (Phase 0). The project skeleton, typed IPC and CI are in place; features land next. Progress is tracked in [docs/Task.md](docs/Task.md).

## Documentation

| Document                                                    | What's in it                                                                                                   |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| [Product requirements (PRD)](docs/PRD.md)                   | Problem, users, requirements with IDs (CTX, LIS, MEM, QA, SCR, MOD, SKL, UI, KEY), consent and ethics, roadmap |
| [Technical documentation](docs/TECHNICAL.md)                | Architecture, data model, IPC contract, setup, coding standards, testing, release, security, operations        |
| [Implementation tasks](docs/Task.md)                        | Every task to build Sidekick end to end, its tests and its current status                                      |
| [Phase 0 manual checks](docs/phase-0-manual-checks.md)      | Step-by-step hardware checks that close Phase 0                                                                |
| [Architecture decision records](docs/adr/README.md)         | One file per significant decision (ADR-001 onwards)                                                            |
| [Module boundaries](docs/TECHNICAL.md#78-module-boundaries) | How the code is split and the rules that keep it modular                                                       |
| [User guide](docs/TECHNICAL.md#12-user-guide)               | Install, first run, features, shortcuts, FAQs                                                                  |

The technical doc is the single source of truth for engineering: when code and doc disagree, fix whichever is wrong in the same pull request.

## Tech stack

Tauri 2 desktop app for Windows and macOS. Rust core (Cargo workspace, one crate per module) and a React 19 + TypeScript frontend, connected by typed IPC generated with tauri-specta. No hosted backend: the app calls the speech-to-text, LLM and search providers you choose, with your own API keys stored in the OS keychain.

## Quick start

Prerequisites are in [technical doc section 6.1](docs/TECHNICAL.md#61-prerequisites) (Rust, Node 22+, pnpm, platform build tools).

```bash
pnpm install
```

```bash
pnpm tauri dev
```

To work on the UI in a browser with a mocked backend:

```bash
pnpm dev:web
```

To run every check CI runs:

```bash
pnpm verify
```

All commands are listed in [section 6.5](docs/TECHNICAL.md#65-run-build-and-test).

## Repository layout

```text
src/            React frontend: app shell, features/<name>, ipc (the only route to Rust)
src-tauri/      Thin Tauri shell: IPC commands, capabilities, app config
crates/         Rust domain crates, one per module (core, store, stt, gateway, ...)
docs/           PRD, technical documentation, ADRs
.github/        CI workflows
```

The full tree is in [section 7.1](docs/TECHNICAL.md#71-repository-structure).

## Contributing

Work on short-lived branches (`feat/...`, `fix/...`, `chore/...`, `docs/...`) with [Conventional Commits](https://www.conventionalcommits.org/), and open a pull request even when working solo. The checklist is in [section 7.4](docs/TECHNICAL.md#74-code-review-and-pull-requests). New documents go in `docs/` and get a row in the table above.

## Responsible use

Sidekick processes other people's voices, so it asks you to confirm participants are aware of transcription before it listens to anyone but you. Interview features are for practice and preparation, not live hiring assessments or exams. See [PRD section 8](docs/PRD.md#8-trust-consent-legal-and-ethical-considerations).
