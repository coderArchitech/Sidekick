# Architecture decision records

One file per significant decision. Summaries are kept in [TECHNICAL.md, section 3.5](../TECHNICAL.md#35-architecture-decision-records).

To add one: copy [template.md](template.md) to the next number, and add a row to section 3.5 in the same pull request as the code change.

| ADR                                                                | Decision                                                                                   | Status   |
| ------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ | -------- |
| [ADR-001](0001-tauri-2-for-the-desktop-app.md)                     | Tauri 2 for the desktop app                                                                | Accepted |
| [ADR-002](0002-no-hosted-backend.md)                               | No hosted backend; app calls providers directly                                            | Accepted |
| [ADR-003](0003-orchestration-in-typescript-platform-in-rust.md)    | Orchestration and prompts in TypeScript; OS, storage and network in Rust                   | Accepted |
| [ADR-004](0004-sqlite-sqlite-vec-sqlcipher.md)                     | SQLite + sqlite-vec + SQLCipher for all local data                                         | Accepted |
| [ADR-005](0005-deepgram-nova-3-default-stt.md)                     | Deepgram Nova-3 as default STT, behind a provider trait                                    | Accepted |
| [ADR-006](0006-os-level-audio-capture.md)                          | OS-level audio capture                                                                     | Accepted |
| [ADR-007](0007-capture-exclusion-via-os-apis.md)                   | Capture exclusion via OS APIs, with visible status and fallback (auto-hide, second device) | Accepted |
| [ADR-008](0008-api-keys-in-os-keychain-only.md)                    | API keys in OS keychain only                                                               | Accepted |
| [ADR-009](0009-skills-as-markdown-files-with-yaml-front-matter.md) | Skills as Markdown files with YAML front matter                                            | Accepted |
| [ADR-010](0010-webdriverio-e2e-playwright-ui-tests.md)             | WebdriverIO for full-app E2E; Playwright for UI-only tests                                 | Accepted |
| [ADR-011](0011-cargo-workspace-one-crate-per-module.md)            | Cargo workspace with one crate per Rust module                                             | Accepted |
| [ADR-012](0012-frontend-feature-modules-with-public-index.md)      | Frontend feature modules with a public index, enforced by lint                             | Accepted |
