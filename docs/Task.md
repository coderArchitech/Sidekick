# Sidekick: Implementation Tasks

The working task list for building Sidekick end to end, derived from the [technical documentation](TECHNICAL.md) and [PRD](PRD.md). Updated in the same commit as the work it tracks.

## How to use this file

**Statuses** (only the [board](#board) holds status; task details below hold scope and tests):

| Status | Meaning |
| --- | --- |
| ⬜ To do | Not started |
| 🟡 In progress: writing code | Implementing the feature |
| 🟡 In progress: writing tests | Writing the tests listed for the task |
| 🟡 In progress: testing | Running tests, fixing failures, manual checks |
| ✅ Done | Definition of done met (below) |
| ⛔ Blocked | Waiting on something; the reason is in the Notes column |

**Definition of done** for every task:

1. Code written, following the module boundaries in [TECHNICAL.md 7.8](TECHNICAL.md#78-module-boundaries).
2. Every test listed under the task is written and passing; `pnpm verify` is green locally.
3. CI is green on Windows and macOS (once CI runs on pull requests, see T-002).
4. Docs updated: TECHNICAL.md (and an ADR if a significant decision was made), IPC tables if commands or events changed, crate status in TECHNICAL.md 3.2.
5. Tasks marked 🖥️ are tested on a real macOS machine before closing. Their Windows hardware check is its own task in [Phase 5: Windows testing](#phase-5-windows-testing), so a feature task doesn't stay open waiting for Windows.

**Test levels** used below: **Unit** (cargo test / Vitest), **Integration** (cargo test in `tests/`, mock HTTP with wiremock), **UI** (Playwright with mocked IPC), **E2E** (WebdriverIO on the real app with mock providers), **Manual** (checklist on real machines). See [TECHNICAL.md 8.1](TECHNICAL.md#81-test-strategy).

**Adding a task:** take the next free ID, add a board row and a detail section in the same phase. Don't renumber existing tasks.

## Board

### Phase 0: Foundation and risk retirement

All Phase 0 code and automated tests are done and green on Linux, Windows and macOS. T-008 to T-012 were closed after macOS testing; their Windows hardware checks are tracked in [Phase 5: Windows testing](#phase-5-windows-testing).

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-001 | Project scaffold | ✅ Done | Branch `chore/scaffold` |
| T-002 | CI green on Windows and macOS | ✅ Done | [PR #1](https://github.com/coderArchitech/Sidekick/pull/1) green on Linux, Windows, macOS |
| T-003 | Apply doc fixes P1–P5 | ✅ Done | Applied with recommended defaults; ADR-013 |
| T-004 | `secrets` crate: OS keychain | ✅ Done | Real keychain round trip passes on Windows and macOS CI |
| T-005 | `store` crate: encrypted database and migrations | ✅ Done |  |
| T-006 | Settings storage and commands | ✅ Done |  |
| T-007 | `gateway` crate: LLM provider trait, mock and Anthropic | ✅ Done |  |
| T-008 | Settings window: providers and API keys | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Settings Playwright (WebKit), real-app E2E add/test/remove key, real keychain round trip. Not run on macOS: save with a real Anthropic key. Windows: T-052. Report: [macOS test report](Macos_testing_report.md#t-008-settings-window-providers-and-api-keys) |
| T-009 | `audio` crate: microphone and system audio capture | ✅ Done | Closed after macOS testing (2026-11-09). Passed: microphone and system-audio device tests on macOS 26.7 (system audio with audio playing, as the check specifies). Not run: live-call glitch check, denied-permission message. Windows: T-053. Report: [macOS test report](Macos_testing_report.md#t-009-audio-capture-microphone-and-system-audio) |
| T-010 | `stt` crate: Deepgram streaming | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Deepgram adapter tests (local WebSocket server). Live latency with a real key carried by T-039. Windows: T-054. Report: [macOS test report](Macos_testing_report.md#t-010-deepgram-streaming) |
| T-011 | `window` crate: capture exclusion and protection status | ✅ Done | Closed after macOS testing (2026-11-09). Passed: shield and Diagnostics protection states in Playwright (WebKit), window crate tests. Not run: hiding in live meeting apps with a second viewer, ADR-015 window titles. Windows: T-055. Report: [macOS test report](Macos_testing_report.md#t-011-capture-exclusion-and-protection-status) |
| T-012 | `diagnostics` crate: latency metrics and baselines | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Diagnostics Playwright (WebKit) and Rust tests. Reference-laptop baselines carried by T-038. Windows: T-056. Report: [macOS test report](Macos_testing_report.md#t-012-latency-metrics-and-baselines) |

### Phase 1: MVP (all P0 requirements)

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-013 | Sessions and consent | ✅ Done | E2E passes with the T-033 suite |
| T-014 | Live transcript tab | ✅ Done | Verified in the real app with fixture audio |
| T-015 | `hotkeys` crate: global shortcuts | ✅ Done | Closed after macOS testing (2026-11-09). Passed: shortcut-driven flows in Playwright (WebKit: Auto, Ask, click-through) and Rust validation. Not run: shortcuts while a meeting app has focus. Windows: T-057. Report: [macOS test report](Macos_testing_report.md#t-015-global-shortcuts) |
| T-016 | Profile: name and nicknames | ✅ Done | Detection use lands in T-020 |
| T-017 | `ingest` crate: parse, chunk, embed | ✅ Done | Closed after macOS testing (2026-11-09). Passed: ingest Rust tests and library flows in Playwright (WebKit). Image files through a vision model carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-017-ingest-parse-chunk-embed) |
| T-018 | `retrieval` crate: hybrid search and context packs | ✅ Done | 20k-chunk search 8 ms in release; latest screenshot joined with T-025 |
| T-019 | Library feature: documents in a session | ✅ Done | Closed after macOS testing (2026-11-09). Passed: `library.spec.ts` in WebKit (picker, drop, refusal, retry, links, axe). Not run: native macOS picker and Finder drag. Windows: T-058. Report: [macOS test report](Macos_testing_report.md#t-019-library-documents-in-a-session) |
| T-020 | Orchestrator: question detection and modes | ✅ Done | Rules meet the targets on a never-tuned set (95% / 100%); real-meeting check in T-039 |
| T-021 | Orchestrator: prompts, Skills, two-tier answers | ✅ Done | Closed after macOS testing (2026-11-09). Passed: answers, Skills and agents in Playwright (WebKit) and real-app E2E. Live-model quality carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-021-prompts-skills-two-tier-answers) |
| T-022 | Live Answers tab | ✅ Done | Follow-up chips not in scope; Playwright pass in T-032 |
| T-023 | Chat tab | ✅ Done | Voice input to Chat later; Playwright pass in T-032 |
| T-024 | Rolling notes | ✅ Done | Summary written by the overlay after End; quality checked in T-039 |
| T-025 | `capture` crate: screenshots and answer from screen | ✅ Done | Closed after macOS testing (2026-11-09). Passed: capture-and-ask in Playwright (WebKit) and real-app E2E. Not run: capture during a live call, Screen Recording prompt. Windows: T-059. Report: [macOS test report](Macos_testing_report.md#t-025-screenshots-and-answer-from-screen) |
| T-026 | More LLM providers: OpenAI, Gemini, Ollama | ✅ Done | Default model names may need refreshing as providers release new ones; live-key check in T-039 |
| T-027 | More STT providers: AssemblyAI, Soniox, local Whisper | ✅ Done | Closed after macOS testing (2026-11-09). Passed: adapter Rust tests, Whisper model list and download in Playwright (WebKit). Live AssemblyAI/Soniox, Whisper accuracy and Hinglish carried by T-039. Windows: T-060. Report: [macOS test report](Macos_testing_report.md#t-027-stt-providers-assemblyai-soniox-local-whisper) |
| T-028 | `skills` crate and Skill builder | ✅ Done | Golden cases run against live models in T-039; tool use itself comes with Agents (T-045) |
| T-029 | Overlay controls and private display | ✅ Done | Closed after macOS testing (2026-11-09). Passed: `overlay-controls.spec.ts` in WebKit, placement unit tests. Not run: two-monitor restore (one display attached). Windows: T-061. Report: [macOS test report](Macos_testing_report.md#t-029-overlay-controls-and-private-display) |
| T-030 | Data control: retention, delete session | ✅ Done | Also fixed: deleting a session now deletes its vectors (they have no foreign key) |
| T-031 | Logging to file and log export | ✅ Done | Log scan runs after every E2E pass |
| T-032 | UI test suite (Playwright) | ✅ Done | 16 specs; UI job green in CI. axe found and fixed: A−/A+ names, low-contrast grey text |
| T-033 | End-to-end test suite (WebdriverIO) | ✅ Done | Green on Ubuntu, Windows (WebView2) and macOS (WebKit) in CI; one suite on every PR (about a minute once built), no separate nightly |
| T-034 | MVP acceptance pass | ⬜ To do | 🖥️ TECHNICAL.md 2.6 |

### Phase 2: V1 launch readiness

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-035 | Usage and cost meter | ⬜ To do | |
| T-036 | Backup and restore | ⬜ To do | |
| T-037 | Release pipeline: signing and auto-update | ⬜ To do | Needs signing certificates |
| T-038 | Performance soak and baselines | ⬜ To do | Also carries the T-012 baselines on the 8 GB reference laptops |
| T-039 | Answer quality eval | ⬜ To do | Needs real provider keys. Also carries the live-key and real-model checks of T-010, T-017, T-021, T-027, T-041 to T-047 (moved here when those tasks closed after macOS testing) |
| T-040 | Release checklist and first beta | ⬜ To do | 🖥️ |

### Phase 3: P1 features (ship if time allows)

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-041 | Meeting memory extras: agenda, recap, action items | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit) and real-app E2E. Real-model quality carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-041-meeting-memory-agenda-recap-action-items) |
| T-042 | Answer follow-up chips | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit) and real-app E2E. Real-model check carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-042-answer-follow-up-chips) |
| T-043 | Screenshot quick actions and strip | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit) and real-app E2E. Real-model check carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-043-screenshot-quick-actions-and-strip) |
| T-044 | Pre-meeting brief and Knowledge Packs | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit) and real-app E2E. Brief quality carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-044-pre-meeting-brief-and-knowledge-packs) |
| T-045 | Agents (tools) and multiple active Skills | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit) and real-app E2E. Real-model tool choice carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-045-agents-and-multiple-active-skills) |
| T-046 | Web search provider and per-session overrides | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit), real-app E2E, Rust mock-HTTP tests. Live Tavily and Brave check carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-046-web-search-and-per-session-overrides) |
| T-047 | Custom vocabulary | ✅ Done | Closed after macOS testing (2026-11-09). Passed: Playwright (WebKit), real-app E2E. Live accuracy check carried by T-039. Report: [macOS test report](Macos_testing_report.md#t-047-custom-vocabulary) |
| T-048 | Click-through mode and themes | ✅ Done | Closed after macOS testing (2026-11-09). Passed: axe per theme and click-through in Playwright (WebKit) and real-app E2E. Not run: physical clicks through to another app. Windows: T-062. Report: [macOS test report](Macos_testing_report.md#t-048-click-through-mode-and-themes) |
| T-049 | Glass overlay: translucent background, crisp text | ✅ Done | Closed after macOS testing (2026-11-09). Passed: `glass.spec.ts` in WebKit, real-app E2E. Not run: a look over a live call. Windows: T-063. Report: [macOS test report](Macos_testing_report.md#t-049-glass-overlay) |

### Phase 4: macOS verification

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-051 | macOS device verification | ✅ Done | Tested 2026-11-09 on macOS 26.7 (arm64). Playwright 53/53 in WebKit and Chromium, Vitest 277/277, `cargo test --workspace` 292/292, real-app E2E 13/13, keychain, microphone and system-audio device tests. Every in-progress task closed; what wasn't run is listed per task in [Phase 4 details](#phase-4-details). Report with screenshots: [Macos_testing_report.md](Macos_testing_report.md) |

### Phase 5: Windows testing

The Windows half of every hardware check, split out when the macOS half closed. Run on a Windows 10 and a Windows 11 laptop (the 8 GB reference machine for baselines). Steps in [Phase 5 details](#phase-5-details).

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-050 | Windows setup and automated suite | 🟡 In progress: testing | 🖥️ `pnpm verify` and `pnpm test:e2e` green on a real Windows 10 and 11 machine (CI already runs them on Windows Server) |
| T-052 | Windows: Settings keys in the real app (T-008) | 🟡 In progress: testing | 🖥️ Key saved, survives restart, removed from Credential Manager |
| T-053 | Windows: audio capture (T-009) | 🟡 In progress: testing | 🖥️ Ignored audio tests pass; no glitches in a real call |
| T-054 | Windows: Deepgram latency (T-010) | 🟡 In progress: testing | 🖥️ p50 under 1,000 ms, p95 under 1,500 ms; needs a Deepgram key |
| T-055 | Windows: screen-share hiding (T-011) | 🟡 In progress: testing | 🖥️ Teams, Zoom, Meet, Webex, Slack; full screen and window; ADR-015 titles |
| T-056 | Windows: baselines (T-012) | 🟡 In progress: testing | 🖥️ Cold start, `.msi` size, idle CPU and RAM on the 8 GB laptop |
| T-057 | Windows: shortcuts with a meeting app focused (T-015) | 🟡 In progress: testing | 🖥️ |
| T-058 | Windows: native picker and Explorer drag-and-drop (T-019) | 🟡 In progress: testing | 🖥️ |
| T-059 | Windows: screenshots during a live call (T-025) | 🟡 In progress: testing | 🖥️ No meeting-app notification; window and area capture |
| T-060 | Windows: STT providers (T-027) | 🟡 In progress: testing | 🖥️ AssemblyAI, Soniox, local Whisper; needs keys |
| T-061 | Windows: two-monitor position restore (T-029) | 🟡 In progress: testing | 🖥️ |
| T-062 | Windows: click-through (T-048) | 🟡 In progress: testing | 🖥️ |
| T-063 | Windows: glass over a live call (T-049) | 🟡 In progress: testing | 🖥️ |

---

## Phase 0 details

### T-001 Project scaffold

**Refs:** TECHNICAL.md 3.4, 7.1, 7.8; ADR-011, ADR-012
**Scope:** Cargo workspace with `sidekick-core` and the `sidekick-app` shell; React + TypeScript frontend with the overlay feature; typed IPC via tauri-specta (`app_info`); ESLint boundary rules; CI workflow; docs in `docs/`.

**Tests**
- [x] Unit (Rust): `SidekickError` retryable flag follows the code; JSON wire format is stable.
- [x] Unit (Rust): `app_info` reports the package version; bindings export succeeds.
- [x] Component: overlay shows four tabs, switches tabs, shows version from (mocked) IPC; opacity clamps to 20–100%.
- [x] Lint: boundary rules reject deep feature imports, direct `invoke`, and `@/ipc/mock` imports.
- [x] Manual: debug app launches on Linux and renders the overlay with the version from Rust.

### T-002 CI green on Windows and macOS

**Refs:** TECHNICAL.md 9.1, 14.1
**Scope:** Open the scaffold PR so `ci.yml` runs; fix any Windows or macOS build issues; record the result in TECHNICAL.md 14.1.

**Tests**
- [ ] CI: Frontend job green.
- [ ] CI: Rust job green on ubuntu, windows and macos.
- [ ] CI: IPC bindings drift check green, and fails when `bindings.ts` is edited by hand (verify once on a throwaway commit).

### T-003 Apply doc fixes P1–P5

**Refs:** TECHNICAL.md 14.6
**Scope:** Once approved, update the named sections: separate `EmbeddingProvider` and choose the embeddings model and vector size (P1); `web_search` command (P2); per-question concurrency (P3); FTS sync triggers (P4); name vocabulary as P0 (P5). Add ADRs where the decision is significant (P1).

**Tests**
- [x] Doc link check passes (all relative links and anchors resolve).
- [ ] Tests for each change are added in the task that implements it (T-005, T-007, T-010, T-017, T-021).

### T-004 `secrets` crate: OS keychain

**Refs:** KEY-5, KEY-6, KEY-7; TECHNICAL.md 3.2, 11.4, NFR-7; ADR-008
**Scope:** `crates/secrets`: `set`, `get`, `delete`, `status` for `sidekick/<provider>` entries; keys held as `SecretString`; `mask()` returns only the last 4 characters; a `KeyStore` trait so other crates and tests can use an in-memory store.

**Tests**
- [x] Unit: `last4()` shows only the last 4 characters, handles keys shorter than 4 and multi-byte text.
- [x] Unit: in-memory store set/get/delete/overwrite; multiple providers kept independently.
- [x] Unit: `SecretString` values never appear in `Debug` output; `KeyStatus` JSON holds only the last 4.
- [x] Unit: account names restricted to `[a-z0-9-]`, max 64.
- [x] Integration: real keychain round trip (`--ignored`) passes on Linux, Windows and macOS CI.
- [x] Unit: keychain errors map to `SidekickError` codes, message contains no key.

### T-005 `store` crate: encrypted database and migrations

**Refs:** TECHNICAL.md 4.1–4.6, NFR-8; ADR-004
**Scope:** `crates/store`: rusqlite with bundled SQLCipher; database key generated on first run and kept in the keychain via `secrets`; `rusqlite_migration` with `0001_init.sql`; WAL and foreign keys on; pre-migration backup (keep 3); repository functions per table as features need them; sqlite-vec loaded.

**Tests**
- [x] Integration: fresh database migrates to the latest `user_version`; reopen keeps data.
- [x] Integration: opening with the wrong key or without a key fails; no plaintext or SQLite header in the file (NFR-8).
- [x] Unit: an upgrade backs up first; a failing migration rolls back and keeps the backup; only 3 backups are kept.
- [x] Integration: foreign keys enforced; deleting a session cascades to all 10 child tables.
- [x] Integration: sqlite-vec loads; nearest-neighbour query returns the expected rows; upsert replaces.
- [x] Integration: changing embedding size recreates the vector table (ADR-013).
- [x] Integration: FTS stays in sync with `chunks` on insert, update and delete (P4).
- [x] Unit: database key created once (64 hex chars), reused, random per install.
- Moved to T-007: no API key is ever written to any table (needs the key commands).

### T-006 Settings storage and commands

**Refs:** KEY-1, KEY-6; TECHNICAL.md 4.5 (seed data), 5.1 (`get_settings`, `update_settings`)
**Scope:** Typed `Settings` struct with defaults (Deepgram, Auto-answer on, retention 30 days); stored in the `settings` table; `get_settings` and `update_settings(patch)` commands; settings window capability.

**Tests**
- [x] Unit: defaults match TECHNICAL.md 4.5 and ADR-013.
- [x] Unit: partial patch updates only the given sections; unknown keys rejected; invalid provider ids rejected.
- [x] Unit: `search` absent leaves it, `null` clears it; retention uses the documented wire values.
- [x] Unit: a corrupt stored section falls back to its default.
- [x] Integration: settings persist across store reopen; app state creates the key and database on first run and reuses them.
- [x] Unit: capability files grant settings commands to the Settings window only; every declared command is granted to some window.
- [x] Component: browser mocks mirror the Rust patch rules.
- [x] Manual: app starts with `SIDEKICK_DATA_DIR` and mock providers, creates and migrates the database.

### T-007 `gateway` crate: LLM provider trait, mock and Anthropic

**Refs:** MOD-1, MOD-2, KEY-3, KEY-4; TECHNICAL.md 5.1 (`llm_stream`, `cancel_stream`, `list_models`, `set_api_key`, `test_api_key`, `delete_api_key`, `list_providers`), 5.2 (`answer.*` events), 5.5, 5.6
**Scope:** `LlmProvider` trait in `sidekick-core`; provider registry with `ProviderInfo`; mock provider (`SIDEKICK_MOCK_PROVIDERS=1`); Anthropic adapter with streaming; retries with backoff and `Retry-After`; cancellation; key commands that test then store keys.

**Tests**
- [x] Unit: Anthropic request building (system prompt, messages, model, effort); no sampling parameters; refusal fallback only on supported models. Tools move to T-021/T-046.
- [x] Unit: SSE parser handles split chunks, CRLF, multi-line data, comments.
- [x] Unit: stream parsing into token deltas, usage and finish reason; thinking blocks skipped; `refusal` surfaced; in-stream errors mapped without echoing provider text.
- [x] Unit: error mapping 401/403 → `KEY_INVALID`, 429 → `RATE_LIMITED`, 5xx/529 → `PROVIDER_UNAVAILABLE`; `Retry-After` parsed.
- [x] Unit: retries 3 times with ±20% jitter, honours `Retry-After`, gives up when it's too long, never retries non-retryable errors.
- [x] Integration (wiremock): correct headers; 429 twice then success; 500 four times then `PROVIDER_UNAVAILABLE`.
- [x] Integration: `cancel_stream` stops token events and reports `CANCELLED`; no tokens after cancel.
- [x] Integration: `set_api_key` stores only after a successful test (401 not saved, not retried); responses and provider lists never include the key; providers without an adapter save unverified.
- [x] Integration: missing key reported before streaming; mock mode answers without keys.
- [x] Integration: no API key in any database table after saving one (moved from T-005).
- [x] Unit: capability files give key commands to the Settings window only, streaming to the overlay.
- [x] Component: browser mocks stream tokens, cancel, and mirror key rules.

### T-008 Settings window: providers and API keys

**Refs:** KEY-1 to KEY-7; TECHNICAL.md 3.7 (Settings window), 12.1
**Scope:** Second Tauri window; `settings` feature with Speech-to-text, LLM and Web search sections; provider and model dropdowns; masked key field with show/hide and paste; Test connection; Remove key; last 4 characters after save.

**Tests**
- [x] Component: one section per kind with defaults selected; dropdowns list providers from `list_providers`; model dropdown appears once a key is saved and the choice persists.
- [x] Component: key field masked by default, show/hide toggles (`aria-pressed`), paste from clipboard works.
- [x] Component: Save and test / Test connection show success; `KEY_INVALID` and network errors have distinct text (by error code).
- [x] Component: after save the input is cleared and only the last 4 characters show; Remove key returns to the input.
- [x] Component: switching active provider needs no re-entry when a key is saved; web search turns on and off; general settings save.
- [x] Component: Close hides the window; the overlay's Settings button opens it; the app routes by `?window=`.
- [x] Accessibility (axe): no violations (colour contrast checked manually).
- [x] Manual: page checked in a real browser with mocks (screenshot in the PR).
- [ ] Manual: Settings window opens from the overlay in the real app on Windows and macOS; keys reach the keychain.

### T-009 `audio` crate: microphone and system audio capture 🖥️

**Refs:** LIS-1, LIS-4, LIS-6; TECHNICAL.md 3.2, 6.4 (`SIDEKICK_AUDIO_FIXTURE`), NFR-3, NFR-6; ADR-006
**Scope:** microphone and system audio via cpal 0.18 (WASAPI loopback on Windows, Core Audio tap on macOS 14.6+, ADR-014); resample to 16 kHz mono PCM in 50 ms frames; energy VAD; WAV fixture source; pause, resume, stop; never writes audio to disk.

**Tests**
- [x] Unit: resampler converts 48 kHz stereo (and 44.1/32/16 kHz) to 16 kHz mono, keeping pitch and level; content above 8 kHz attenuated below -38 dB.
- [x] Unit: VAD marks silence, speech, and the 300 ms hangover; absorbs steady hum; keeps 10 s of continuous speech as speech.
- [x] Unit: pipeline emits 800-sample frames with 50 ms timestamps.
- [x] Integration: fixture source streams a WAV file (fast and real time) with correct speech flags.
- [x] Integration: pause drops frames (time still passes); resume continues; stop closes the stream.
- [x] Integration: no files are created during capture (NFR-6).
- [x] Unit: system audio on Linux reports `PERMISSION_DENIED` clearly.
- [ ] Manual (Windows, macOS 14.6+): `cargo test -p sidekick-audio -- --ignored` with a mic and audio playing.
- [ ] Manual (Windows, macOS): mic and system audio captured as separate streams during a Teams, Zoom and Meet call, with no dropouts in the meeting app.
- [ ] Manual (macOS): clear error when System Audio Recording permission is missing (`PERMISSION_DENIED`), and on macOS 13–14.5.

### T-010 `stt` crate: Deepgram streaming

**Refs:** LIS-2, LIS-3; KEY-2; TECHNICAL.md 5.2 (`transcript.*`, `stt.status`), 5.4 (`STT_DISCONNECTED`), NFR-1, NFR-4; ADR-005
**Scope:** `SttProvider` trait in `sidekick-core`; Deepgram Nova-3 WebSocket adapter with interim and final results, diarization for the system stream; mic stream labelled "Me"; reconnect within 3 s with 10 s audio buffer; mock provider replaying a fixture transcript.

**Tests**
- [x] Unit: listen URL (model, encoding, diarize for system audio only, keyterms); message parsing into partial and final results; metadata and empty results ignored.
- [x] Unit: mic segments labelled `me` / `is_user`; system segments split per speaker run as `speaker_N`.
- [x] Unit: timeline maps provider time to meeting time across skipped silence.
- [x] Integration (local WebSocket server): auth header sent; final result becomes a segment with meeting timestamps; `CloseStream` on end.
- [x] Integration: silence isn't sent beyond a 500 ms tail; keep-alives sent during silence.
- [x] Integration: connection drop → `Reconnecting` → `Live` within 3 s; unfinalised and buffered audio replayed, nothing lost.
- [x] Integration: 401 maps to `KEY_INVALID`, `Failed`, no retry; unreachable server gives up after the deadline; cancel stops the run; missing key reported.
- [x] Integration: Deepgram key verifier (wiremock 200 / 401); gateway uses registered verifiers before saving keys.
- [x] Integration: mock provider turns speech bursts in fixture frames into segments.
- [x] Tool: `cargo run -p sidekick-stt --example live_transcribe` streams live audio to Deepgram and reports lag p50/p95 (key read from the keychain).
- [ ] Manual: transcript lag measured against NFR-1 with a real key on a real call; result recorded in TECHNICAL.md 13.2.

### T-011 `window` crate: capture exclusion and protection status 🖥️

**Refs:** UI-4, UI-4a, UI-4b; TECHNICAL.md 5.1 (`get_capture_protection`), 5.2 (`capture.protection`, `screen_share.detected`), NFR-9; ADR-007
**Scope:** Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`; macOS window sharing type none; status check per OS version; auto-hide fallback when screen sharing is detected and exclusion isn't guaranteed; `SIDEKICK_DISABLE_EXCLUSION`.

**Tests**
- [x] Unit: status logic returns protected / not guaranteed / unavailable per OS and version (Windows build 19041, macOS 15), with a reason for each.
- [x] Unit: auto-hide rule applies only while sharing, with auto-hide on, and protection not guaranteed.
- [x] Unit: disabled flag and failure to apply produce `disabled` / `unavailable`.
- [x] Component: shield shows each state with colour, label and tooltip reason.
- [x] UI (`tests/ui/protection.spec.ts`): header shield says "Visible in screen share" in a browser; Diagnostics shows the reason for protected, not guaranteed and disabled.
- [x] Manual: real app on Linux applies protection at startup and shows the red shield.
- [x] Screen-share detection design: [ADR-015](adr/0015-screen-share-detection.md) (proposed) detects meeting apps' own share windows; manual hide hotkey (T-015) stays as the fallback.
- [ ] Validate ADR-015: record the top-level window lists while sharing in each meeting app on Windows and macOS (do this during the manual check below), then implement it as its own task.
- [ ] Manual (Windows 10, 11; macOS 14, 15, latest): overlay not visible to others when sharing full screen and a window in Teams, Zoom, Meet, Webex, Slack huddles; results recorded in a table in TECHNICAL.md 8.5; adjust the status rules to match.

### T-012 `diagnostics` crate: latency metrics and baselines

**Refs:** TECHNICAL.md 10.1, 13.1, 13.2; NFR-1, NFR-2
**Scope:** Record per-stage timings (end of speech, segment, detection, retrieval, first token); p50/p95 over a rolling window; `get_diagnostics` command; Diagnostics panel in Settings.

**Tests**
- [x] Unit: nearest-rank percentile calculation on known samples.
- [x] Unit: rolling window drops samples older than 15 minutes; at most 1,000 kept.
- [x] Unit: health against targets; no verdict without samples.
- [x] Integration: the gateway records LLM time-to-first-token.
- [x] Component: Diagnostics panel shows every stage, p50/p95, Healthy / Slow / No data, and refreshes.
- [x] UI (`tests/ui/diagnostics.spec.ts`): latency table lists all five stages with targets, passes axe; log export reports success and does nothing when cancelled.
- [ ] Manual: Phase 0 baselines recorded in TECHNICAL.md 13.2 on the reference Windows and macOS laptops (needs T-013 for end-to-end numbers).

## Phase 1 details

### T-013 Sessions and consent

**Refs:** CTX-1; PRD 8; TECHNICAL.md 3.3 (session start), 5.1 (`create_session`, `confirm_consent`, `start_session`, `pause_session`, `resume_session`, `end_session`, `delete_session`, `list_sessions`), 5.4 (`CONSENT_REQUIRED`)
**Scope:** `session` and `consent` crates; lifecycle draft → live → paused → ended; consent dialog with "Participants agreed" and "Transcribe me only"; me-only mode starts only the mic stream; consent log kept 1 year.

**Tests**
- [x] Unit: allowed and rejected lifecycle transitions; wire values round-trip.
- [x] Unit: starting without consent returns `CONSENT_REQUIRED`.
- [x] Integration: me-only consent starts mic capture only; system audio never starts. Participants-agreed starts both.
- [x] Integration: system audio unavailable → continues with the mic and a warning; no mic → start fails cleanly.
- [x] Integration: consent log row written with option and time; latest choice wins; only before start; survives session deletion (migration 0002, tested from a real v1 database).
- [x] Integration: `list_sessions` pagination (cursor, default 50, max 200, bad cursor rejected).
- [x] Integration: lifecycle timestamps (first start kept, end recorded); segments saved and deleted with the session.
- [x] Integration: pause drops audio and splits the transcript; stop flushes STT.
- [x] Component: session bar creates, starts, pauses, resumes and ends; consent dialog needs a choice, Cancel/Escape start nothing, focus starts on the first option; errors shown; axe passes.
- [x] Manual: real app (Linux, Xvfb, driven with xdotool) with `SIDEKICK_MOCK_PROVIDERS=1` and `SIDEKICK_AUDIO_FIXTURE`: created a session, consent dialog shown, went live ("Listening"), ended cleanly; no errors in the log.
- [x] E2E: create session → consent → start with fixture audio → transcript appears → end (`tests/e2e/specs/session.e2e.ts`).

### T-014 Live transcript tab

**Refs:** LIS-3, LIS-6; TECHNICAL.md 3.2 (`transcript`), 5.2
**Scope:** Transcript feature subscribing to `transcript.partial` and `transcript.segment`; speaker names with rename and merge; partial text replaced by final; listening indicator; bookmarks list.

**Tests**
- [x] Component: saved lines load in time order; partial text replaced by the final segment; other sessions' events ignored.
- [x] Component: renaming a speaker updates all their lines (and the database); renaming to an existing name merges (Rust test).
- [x] Component: listening indicator reflects `session.state` and pause.
- [x] Component: bookmarks added and listed (Rust test covers ordering and labels).
- [x] Unit: batching every 50 ms keeps flushes to one per interval under a steady stream.
- [x] Manual: real app with fixture audio and mock STT shows live lines in the Transcript tab.

### T-015 `hotkeys` crate: global shortcuts 🖥️

**Refs:** UI-6, LIS-6, QA-3; TECHNICAL.md 12.3
**Scope:** Register shortcuts from settings; emit `hotkey.pressed`; conflicts reported; customisable in Settings → Shortcuts.

**Tests**
- [x] Unit: default bindings match TECHNICAL.md 12.3 (`CmdOrCtrl` = Cmd on macOS, Ctrl on Windows).
- [x] Unit: invalid bindings (no modifier, two keys, unknown key) and duplicates rejected with a clear error naming both actions; spelling normalised.
- [x] Unit: settings store the set, invalid patches change nothing.
- [x] Component: pause/resume and bookmark run on their events; nothing happens without a session; unmount stops listening.
- [x] Component: Settings → Shortcuts lists, saves, rejects duplicates with a reason, and undoes.
- [x] Manual (Linux, Xvfb): Ctrl+Shift+P pauses the session and Ctrl+Shift+H hides the overlay while another window has focus.
- [x] Ask (T-023), Screenshot (T-025) and Auto-answer (T-020) actions wired, with component tests.
- [x] Switch Skill action (T-028): cycles enabled Skills, then none.
- [ ] Manual: shortcuts work while a meeting app has focus on Windows and macOS.

### T-016 Profile: name and nicknames

**Refs:** QA-2; TECHNICAL.md 12.4; T-003 (P5)
**Scope:** Settings → Profile; names and nicknames stored in settings; passed to STT as vocabulary boost and to detection.

**Tests**
- [x] Component: set the name, add (Enter or button), ignore duplicates, remove and save nicknames.
- [x] Unit: names trimmed, blanks and duplicates removed, limits (10 nicknames, 40 characters), persisted.
- [x] Integration: every STT stream of a session gets the names as keyterms; Deepgram adds them as `keyterm` parameters (T-010 test).

### T-017 `ingest` crate: parse, chunk, embed

**Refs:** CTX-2, CTX-3; TECHNICAL.md 3.3 (document upload), 5.1 (`ingest_document`, `delete_document`), 5.2 (`ingest.progress`), 13.5
**Scope:** Parsers for PDF, DOCX, PPTX, XLSX, TXT, MD, images (OCR or vision description), URLs; ~500-token chunks with 50-token overlap and page numbers; duplicate detection by SHA-256; batched embeddings (100 per request, 3 docs in parallel); progress events; `EmbeddingProvider` from T-003.

**Tests**
- [x] Unit: PDF (pages), DOCX (paragraphs, entities), PPTX (slide numbers), XLSX (sheet rows), text and Markdown parsers, on generated fixture files; web pages to text (scripts, nav and tags removed).
- [x] Unit: corrupted file returns `PARSE_FAILED`; unknown type and images return `UNSUPPORTED_FILE`; text-less files fail clearly.
- [x] Unit: chunker sizes (450–500 tokens), overlap (~50 tokens), page attribution, nothing lost.
- [x] Integration: same file twice in a session is detected by hash and not re-embedded; another session gets its own copy.
- [x] Integration: 50-page PDF ingests end to end in 0.1 s with the mock embedder (CTX-3 target 30 s); every chunk has a vector and keyword entry; page attribution checked.
- [x] Integration: progress goes queued → parsing → embedding → ready, percent only rises; failures stored on the document and reported.
- [x] Integration: deleting a document removes its chunks, vectors and keyword rows.
- [x] Integration: OpenAI embeddings adapter (order by index, bearer auth, 401, wrong sizes) and the gateway's embedder choice.
- [ ] Images (OCR or vision description): needs the vision-capable LLM adapters (T-025, T-026).
- [ ] Gemini, Voyage and Ollama embeddings: with the other providers (T-026).

### T-018 `retrieval` crate: hybrid search and context packs

**Refs:** QA-4; TECHNICAL.md 5.1 (`retrieve_context`), 13.1 (100 ms budget)
**Scope:** Vector and FTS search filtered to the session, merged with reciprocal rank fusion; context pack = last 3 minutes of transcript + notes + top 5 chunks + latest screenshot; retrieval order transcript → documents → web.

**Tests**
- [x] Unit: rank fusion ordering on known inputs (items in both lists first; deterministic ties).
- [x] Unit: transcript window trimming to 3 minutes; safe FTS queries (quoted words, stop words and operators dropped).
- [x] Integration: 5 golden queries return the expected document first (top 5 returned), with vector + keyword and keyword-only.
- [x] Integration: results never include another session's documents (vector index partitioned by owner; keyword search filtered).
- [x] Integration: pack carries recent transcript, notes, top-k passages with document name, id and page.
- [x] Benchmark: 20k chunks, release build: vector search 8 ms (target 50 ms), full context pack 23 ms (budget 100 ms). Debug: 17 / 91 ms.
- [x] Latest screenshot in the pack (last 10 minutes, this session only): added with T-025.

### T-019 Library feature: documents in a session

**Refs:** CTX-1, CTX-2; TECHNICAL.md 12.2
**Scope:** Drag-and-drop and file picker; per-document status and errors; delete; URL input.

**Tests**
- [x] Component: picked files and files dropped onto the window call `ingest_document`; progress events move them to Ready with a count in the header.
- [x] Component: failed documents show the reason and a retry; unsupported files are refused with the reason.
- [x] Component: links added (http/https only, others refused), documents removed.
- [x] UI (`tests/ui/library.spec.ts`): picker, drop onto the window (opens the panel), unsupported file refused, damaged file fails with retry and remove, links validated, axe with a failure showing.
- [ ] Manual: native file picker and OS drag-and-drop on Windows and macOS.

### T-020 Orchestrator: question detection and modes

**Refs:** QA-1, QA-2, QA-3; TECHNICAL.md 5.5 (600 ms debounce), 13.1
**Scope:** Rules for interrogatives and implicit asks; name and nickname matching (fuzzy, tolerant of STT spelling); optional small-model classifier; Auto vs Directed-only mode with header button and hotkey; manual asks in both modes; early detection on interim results for directed questions.

**Tests**
- [x] Unit: labelled set of 200 utterances: precision 100%, recall 98%. This set was used to write the rules, so it's optimistic.
- [x] Unit: held-out round 1 (40 lines) scored 69.6% / 80% on the first rules; its mistakes led to general fixes (trust STT punctuation, narrower request patterns, "is there…"), so it's now tuning data (100% / 100%).
- [x] Unit: held-out round 2 (40 lines, written after the fixes, never tuned on): precision 95.2%, recall 100%.
- [x] Unit: name matching with misspellings (edit distance scaled to name length) and nicknames, possessives, multi-word names; no false trigger on similar words.
- [x] Unit: Directed-only answers only directed questions; Auto answers all. Manual asks (Chat) arrive with T-023 and are always answered.
- [x] Unit: 600 ms debounce; split questions merged; the user speaking or another speaker ends a turn; question at the end of a longer turn found; the user's own questions ignored.
- [x] Component: Auto button and hotkey toggle the mode and persist the session setting; detection wired to live transcript events.
- [ ] Real-meeting precision/recall on recorded fixtures (T-039). A small-model classifier stays optional unless that falls short.

### T-021 Orchestrator: prompts, Skills, two-tier answers

**Refs:** QA-4, QA-5, QA-6; MOD-3; TECHNICAL.md 3.3, 5.5, 11.5 (prompt injection), T-003 (P3)
**Scope:** Prompt builder wrapping retrieved content as untrusted data; active Skill instructions; fast cue model and main model in parallel; citation format; "not found in your docs" fallback; at most 2 questions in flight, newest cancels oldest.

**Tests**
- [x] Unit: prompt structure for cue and full variants: rules in the stable system prompt (cache-friendly), numbered transcript-then-document context, Skill section, chat history. Screenshot prompts come with T-025.
- [x] Unit: retrieved text is fenced inside `<context>` and declared as data; an injection string stays inside the fence and never reaches the system prompt.
- [x] Unit: citations `[n]` parsed (first-use order, lists like `[2, 3]`, unknown numbers ignored) into `sources_json` entries (doc page or chunk, transcript time).
- [x] Unit: empty retrieval answers "Not found in your documents or the meeting so far." without calling a model.
- [x] Unit: cue (fast model, low effort, 80 tokens) and full answer (main model, medium effort) stream in parallel; latency recorded per stage; answer saved with tokens and model.
- [x] Unit: a third concurrent question cancels the oldest's streams; its late tokens are ignored; manual cancel works.
- [x] Unit: a stream that fails shows on that answer only and the router keeps working (NFR-5); a failed cue doesn't sink the full answer; refusals show a neutral note.
- [x] Integration (mocks): transcript line → detection → router → saved answer.
- [x] Active Skill selection (T-028): per session, applied by trigger; `full_only` Skills skip the cue.
- [ ] Answer quality with live models: T-039.

### T-022 Live Answers tab

**Refs:** QA-5, QA-6, QA-7; TECHNICAL.md 3.2 (`answers`), 11.5
**Scope:** Cue line, expandable full answer, sources list, code blocks with Shiki highlighting and copy, thumbs up/down (`rate_answer`), sanitised Markdown with no raw HTML.

**Tests**
- [x] Component: cue shows first, expands to full answer, then sources.
- [x] Component: copy button copies code only when clicked (UI-4c).
- [x] Component: raw HTML and script in Markdown are not rendered.
- [x] Component: rating calls `rate_answer` with the right value.
- [x] UI: streamed tokens render progressively with mocked events.

### T-023 Chat tab

**Refs:** QA-8; TECHNICAL.md 12.2
**Scope:** Typed questions with full session context; follow-ups with history; per-session chat history; Ask hotkey focuses the input.

**Tests**
- [x] Component: sending a message streams an answer and keeps history.
- [x] Component: follow-up includes previous turns in the request.
- [x] Integration: chat questions stored with `source = chat`.
- [x] UI: Ask hotkey focuses the chat input.

### T-024 Rolling notes

**Refs:** MEM-1; TECHNICAL.md 3.3 (rolling notes), 5.1 (`save_notes`)
**Scope:** Every 45 s, send new transcript plus current notes to the fast model; save with optimistic version check; Notes tab shows topics, decisions, open questions; summary on session end.

**Tests**
- [x] Unit: job runs every 45 s only while live and only with new transcript.
- [x] Unit: version conflict on `save_notes` retries with the latest notes.
- [x] Component: Notes tab renders sections and updates live.
- [x] Integration: `end_session` produces a summary with decisions and action items (mock model).

### T-025 `capture` crate: screenshots and answer from screen 🖥️

**Refs:** SCR-1 to SCR-4; UI-4c; TECHNICAL.md 5.1 (`capture_screenshot`), 5.2 (`screenshot.captured`), 13.5
**Scope:** Region, window and full-screen capture; downscale to 1568 px long side; encrypted BLOB storage; capture-and-ask; latest screenshot added to context; never copied to clipboard or shared folders.

**Tests**
- [x] Unit: downscale keeps aspect ratio and the 1568 px limit.
- [x] Integration: screenshot stored encrypted and linked to the session.
- [x] Unit: screenshot questions include the image and cite it as a source.
- [x] Component: capture-and-ask flow (capture, type question, answer).
- [ ] Manual: capture during a live call doesn't notify or interrupt the meeting app.

### T-026 More LLM providers: OpenAI, Gemini, Ollama

**Refs:** MOD-1, KEY-3; TECHNICAL.md 3.6, 7.5
**Scope:** Adapters following TECHNICAL.md 7.5, including vision and `list_models`; Ollama without a key on localhost.

**Tests**
- [x] Unit (each provider): request building, stream parsing, error mapping from recorded fixtures.
- [x] Integration (wiremock, each provider): happy path, 401, 429, 500.
- [x] Unit: Ollama unreachable maps to `PROVIDER_UNAVAILABLE` with a helpful message.

### T-027 More STT providers: AssemblyAI, Soniox, local Whisper

**Refs:** KEY-2; LIS-2; TECHNICAL.md 3.6; PRD 11 (Hinglish evaluation)
**Scope:** Adapters behind `SttProvider`; whisper-rs in `spawn_blocking` behind the `local-whisper` feature flag; model download with hash check.

**Tests**
- [x] Unit (each provider): message parsing and error mapping from fixtures; AssemblyAI and Soniox also end to end against a local WebSocket server (auth, results, speakers, finish message, rejected key).
- [ ] Integration: Whisper transcribes the short English fixture within tolerance (opt-in test `crates/stt/tests/whisper.rs`, at least 80% word recall; needs a model and a recording).
- [x] Unit: model download rejects a file with the wrong SHA-256 (and SHA-1, which whisper.cpp publishes) and leaves nothing behind.
- [ ] Manual: Hinglish comparison across providers recorded (PRD open question).

### T-028 `skills` crate and Skill builder

**Refs:** SKL-1, SKL-2, SKL-3; TECHNICAL.md 5.7, 11.5 (malicious Skill); ADR-009
**Scope:** Load and validate Skill files against `schemas/skill.schema.json`; save, delete, import, export; built-in templates seeded on first run; no-code builder UI; import shows requested tools for approval; tool allowlist.

**Tests**
- [x] Unit: valid templates pass schema validation; invalid files give field-level errors.
- [x] Unit: unknown tools are rejected; import lists tools for approval.
- [x] Unit: export then import round trip is lossless.
- [x] Golden: each built-in template has one golden-answer case (TECHNICAL.md 7.5) in `skills/golden/`; a test checks coverage and shape. Scoring with a live model is part of T-039.
- [x] Component: builder creates a Skill from a template and saves it.

### T-029 Overlay controls and private display 🖥️

**Refs:** UI-1 to UI-4c, UI-7; TECHNICAL.md 12.2
**Scope:** Opacity slider (done in T-001), resize, drag, snap to edges, remember position per monitor, font size, compact/expanded, shield indicator wired to T-011, show/hide.

**Tests**
- [x] Unit: snapping and per-monitor position persistence logic (`sidekick_window::placement`, settings validation).
- [x] Component: font size and compact mode change layout and are saved; shield colour follows protection status (T-011 tests).
- [x] Accessibility (axe): overlay has no violations.
- [x] UI (`tests/ui/overlay-controls.spec.ts`): text size stops at 12 and 20 px, Compact hides the session bar and documents and restores them, background slider limited to 20–100%.
- [ ] Manual: position restored across restarts on multi-monitor setups. (Linux, one monitor: moved to 1490,15 → snapped to 1500,0; moved to 700,250, restarted → restored at 700,250.)

### T-030 Data control: retention, delete session

**Refs:** TECHNICAL.md 4.4, 10.7; PRD 6 (data control)
**Scope:** Retention job at startup and every 6 hours; per-session retention setting; delete everything button; VACUUM when idle; consent log kept 1 year.

**Tests**
- [x] Unit: retention cut-off per option (session, 7d, 30d, forever).
- [x] Integration: expired sessions deleted with all child rows; consent log kept.
- [x] Integration: after delete and VACUUM, deleted text isn't found in the database file.

### T-031 Logging to file and log export

**Refs:** TECHNICAL.md 7.7, 10.3, NFR-7
**Scope:** JSON-line logs with daily rotation (14 days, 50 MB); frontend `log` command; correlation IDs; Export logs zip with diagnostics snapshot.

**Tests**
- [x] Unit: rotation (daily, by size) and the 14-day / 50 MB caps; redaction of key-shaped strings.
- [x] Integration: log scan finds no key patterns or fixture phrases after a full E2E run (T-033, runs after every E2E pass). `scripts/scan-logs.mjs` is ready and passed on a real-app run: create, consent, live with fixture audio, Chat question, end, notes and summary.
- [x] Integration: export zip contains logs and diagnostics, no database.

### T-032 UI test suite (Playwright)

**Refs:** TECHNICAL.md 8.1, ADR-010
**Scope:** `tests/ui/` with mocked IPC events; flows: Auto-answer toggle, Chat, code copy, settings dropdowns, consent dialog; axe checks; added to CI.

**Tests**
- [x] UI: each flow above has a spec; axe passes on every screen.
- [x] CI: UI job green.

### T-033 End-to-end test suite (WebdriverIO)

**Refs:** TECHNICAL.md 8.1, 8.3, ADR-010
**Scope:** `tests/e2e/` with @wdio/tauri-service; mock providers and fixture audio; smoke suite on every PR (Windows, macOS), full suite nightly.

**Tests**
- [x] E2E: session start with consent → fixture audio → transcript → detected question → cited answer.
- [x] E2E: screenshot capture and ask.
- [x] E2E: settings: add, test and remove a key (mock provider).
- [x] E2E: LLM failure keeps transcript running (NFR-5).
- [x] CI: suite green on Windows and macOS (and Ubuntu).

### T-034 MVP acceptance pass 🖥️

**Refs:** TECHNICAL.md 2.6
**Scope:** Run every acceptance criterion on Windows and macOS and record the results.

**Tests**
- [ ] Every P0 PRD requirement maps to at least one automated test (traceability table added to TECHNICAL.md).
- [ ] NFR-1, NFR-2, NFR-3 met in a 60-minute recorded test meeting.
- [ ] Directed question about a fact in a 50-page PDF answered correctly with a citation in 8 of 10 trials.
- [ ] Overlay hidden when sharing on Teams, Zoom, Google Meet and Webex.
- [ ] No API key in the database file, logs or crash reports.
- [ ] Consent confirmation blocks listening to others.

## Phase 2 details

### T-035 Usage and cost meter

**Refs:** MOD-4, KEY-10; TECHNICAL.md 4.2 (`usage`), 5.1 (`get_usage`)
**Tests**
- [ ] Unit: cost estimates from configured rates.
- [ ] Integration: STT minutes, tokens and searches recorded per provider and session.
- [ ] Component: usage view totals by provider and range.

### T-036 Backup and restore

**Refs:** TECHNICAL.md 4.6, 10.6
**Tests**
- [ ] Integration: export with passphrase then restore on a fresh data folder gives identical data.
- [ ] Integration: wrong passphrase fails without changing the current database.
- [ ] Integration: backups contain no API keys.

### T-037 Release pipeline: signing and auto-update

**Refs:** TECHNICAL.md 9.1–9.6, NFR-11
**Tests**
- [ ] CI: `release.yml` produces signed Windows MSI/NSIS and notarised universal DMG on a beta tag.
- [ ] CI: installer size under 30 MB.
- [ ] Manual: update from the previous beta installs and keeps data; tampered manifest is rejected.

### T-038 Performance soak and baselines

**Refs:** TECHNICAL.md 13.2–13.4, NFR-1 to NFR-4
**Tests**
- [ ] Soak: 60-minute fixture meeting meets NFR-1 to NFR-3, memory growth under 10%.
- [ ] Long meeting: 3 hours without crash, RAM under 1 GB.
- [ ] Question burst, large library, network flaps and provider failure scenarios pass (TECHNICAL.md 13.3).
- [ ] Results recorded in TECHNICAL.md 13.2 and 13.3.

### T-039 Answer quality eval

**Refs:** TECHNICAL.md 8.1, 8.3, 8.4
**Tests**
- [ ] Golden set in `tests/fixtures/golden/` covers docs, transcript and screenshot questions.
- [ ] Eval script writes `eval-results/<date>.json`; score ≥ 80%.

### T-040 Release checklist and first beta 🖥️

**Refs:** TECHNICAL.md 9.5
**Tests**
- [ ] Every item in the release checklist (TECHNICAL.md 9.5) passes on Windows 10, Windows 11, macOS Apple Silicon and Intel.

## Phase 3 details

Each P1 task gets full scope and tests when it moves to In progress.

| ID | PRD refs | Minimum tests |
| --- | --- | --- |
| T-041 | MEM-2, MEM-3, MEM-4 | Unit: agenda status updates, recap window, action item extraction on fixtures; Component: each view |
| T-042 | QA-9 | Component: each chip sends the right follow-up |
| T-043 | SCR-5, SCR-6 | Unit: each quick action prompt; Component: strip delete and re-ask |
| T-044 | CTX-4, CTX-5 | Integration: brief from fixture docs; packs attach to multiple sessions |
| T-045 | SKL-4, SKL-5 | Unit: tool loop with allowlist and limits; Component: Skill switching |
| T-046 | KEY-8, KEY-9; T-003 (P2) | Unit: Tavily and Brave adapters; Integration: per-session override |
| T-047 | LIS-5 | Unit: vocabulary passed to each STT provider |
| T-048 | UI-5, UI-8 | Manual: click-through on both OSes; axe: each theme passes contrast |

### T-041 Meeting memory extras: agenda, recap, action items

**Refs:** PRD MEM-2, MEM-3, MEM-4; TECHNICAL.md 3.3, 4.2, 6.5
**Scope**
- [x] Agenda per session (`agenda_items`, migration 0004): add or edit one item per line in the Notes tab; each item Not yet / In progress / Covered, settable by hand; "n/m covered".
- [x] The notes job moves the agenda on after each update: fast-model JSON statuses, keyword fallback when the reply can't be read; statuses only move forward; ending the session covers the item in progress.
- [x] "What did I miss?": recap of the last 2, 5 or 10 minutes (counted back from the newest line) with the fast model; no call when nothing was said.
- [x] Action items: live in the rolling notes ("Owner: task (due …)"); stored with owner and due in the summary; can be ticked off (`set_action_item_done`).
**Tests**
- [x] Rust: agenda keeps id and status across edits, limits, unknown ids ignored, deleted with the session; action items tick off.
- [x] Unit (Vitest): keyword matching, model reply parsing, forward-only updates, end-of-meeting close, recap window and prompt, live action items in the notes prompt.
- [x] Component: agenda add/edit/status/error, recap window and dismiss, action item checkbox, axe.
- [x] Playwright (`tests/ui/meeting-memory.spec.ts`): agenda moves and closes when the session ends, recap, action items, axe.
- [x] E2E (`tests/e2e/specs/meeting-memory.e2e.ts`): agenda stored, moved by the notes job and closed on End in the real app; recap of the fixture transcript.
- [ ] Manual 🖥️: agenda and recap quality with a real model (T-039).

### T-042 Answer follow-up chips

**Refs:** PRD QA-9; TECHNICAL.md 7.1 (`answers`)
**Scope**
- [x] Chips under every finished answer in Live Answers and Chat: Shorter, More detail, Give an example, Explain simply. None while streaming or on errors.
- [x] A chip asks through the answer router (`source = chat`): instruction tied to the original question, the original exchange as history, documents searched with the original question.
- [x] The follow-up shows and is saved as "Shorter: question"; the overlay switches to Chat; a chip on a follow-up refers back to the first question.
**Tests**
- [x] Unit: each chip's instruction, label, query and history; chaining; cue fallback; which answers take chips; the router shows/saves the label and searches with the query.
- [x] Component: chips on finished answers only, each chip sends the right follow-up with history and switches to Chat, saved with its label; no session → notice.
- [x] Playwright (`tests/ui/follow-ups.spec.ts`): chip on a live answer → Chat; chips chain in Chat; axe.
- [x] E2E (`tests/e2e/specs/follow-ups.e2e.ts`): Shorter on a detected answer answers in Chat through the real gateway.
- [ ] Manual 🖥️: with a real model, each chip visibly reshapes the answer (T-039).

### T-043 Screenshot quick actions and strip

**Refs:** PRD SCR-5, SCR-6; TECHNICAL.md 6.5 (`retrieve_context`, `delete_screenshot`), 7.1 (`chat`)
**Scope**
- [x] Quick actions on the attached screenshot: Explain, Summarise, Extract table, Answer question on screen, Debug this code. Each sends its own prompt with that exact screenshot.
- [x] `retrieve_context` takes an optional `screenshotId` (this session's only, any age); typed questions about an attached screenshot use it too.
- [x] Screenshot strip in Chat: thumbnails of every capture in the session, reloaded with the session; click to attach again (re-ask), ✕ to delete (`delete_screenshot`).
**Tests**
- [x] Rust: one screenshot by session (never another session's), delete is idempotent; a pack with a chosen screenshot of any age, never another session's.
- [x] Unit: the five actions and their prompts; the router passes `screenshotId` in the context options.
- [x] Component: each action sends its prompt with that screenshot; strip lists, re-asks the exact screenshot, deletes and detaches, reloads with the session; axe.
- [x] Playwright (`tests/ui/screenshot-actions.spec.ts`): a quick action, strip re-ask and delete, axe.
- [x] E2E (`tests/e2e/specs/screenshot-actions.e2e.ts`): quick action, strip, re-ask and delete in the real app.
- [ ] Manual 🖥️: each action on real screenshots (a table, an error, a question) with a vision model (T-039).

### T-044 Pre-meeting brief and Knowledge Packs

**Refs:** PRD CTX-4, CTX-5; TECHNICAL.md 4.2, 6.5, 7.1 (`retrieval`, `library`, `notes`, `settings`)
**Scope**
- [x] Knowledge Packs in Settings: create, add files or links (`Owner::Pack`, the pack's own vector partition), document status, remove documents, rename, delete (with its documents, chunks, vectors and attachments).
- [x] Turn packs on per session in the overlay's Documents panel (`session_packs`, migration 0005); one pack can be on in many sessions.
- [x] Retrieval searches the session's documents plus its attached packs (vector partitions merged by distance; keyword search includes them); other sessions never see them.
- [x] Pre-meeting brief in the Notes tab: Summary, Key numbers, Likely questions from the start of every ready document (session first, then packs, 40k characters shared fairly); saved per session (`briefs`); no documents → a hint, no model call.
**Tests**
- [x] Rust: packs list/rename/limits, attach to several sessions, unknown pack leaves attachments unchanged, delete removes documents, vectors and attachments; attached packs found by vector and keyword search and in the context pack, detached ones and other sessions not; brief material order and budget; brief save/get.
- [x] Unit: brief prompt fencing, main model and sections, no material → no call, empty reply not saved.
- [x] Component: Settings pack create/link/rename/delete and errors, axe; overlay picker toggles and makes retrieval include the pack; brief writes, saves and reloads.
- [x] Playwright (`tests/ui/knowledge-packs.spec.ts`): Settings pack with a link; overlay pack picker, cited pack document in Chat, brief; axe.
- [x] E2E (`tests/e2e/specs/knowledge-packs.e2e.ts`): pack made in Settings and ingested by the real pipeline, turned on in the overlay, cited in Chat, read by the brief.
- [ ] Manual 🖥️: brief quality on real documents with a real model (T-039).

### T-045 Agents (tools) and multiple active Skills

**Refs:** PRD SKL-4, SKL-5; TECHNICAL.md 3.3 (agent tool step), 6.5 (`set_session_extra_skills`), 7.1 (`skills`)
**Scope**
- [x] Agent tool step: a Skill listing tools (allowlist doc_search, web_search, calculator) lets the fast model call them one at a time (JSON) before the answer; provider-agnostic.
- [x] Limits: only the Skill's tools run (anything else is refused and ends the loop), at most 3 calls, 8 s, no repeats; failures never block the answer.
- [x] Calculator without `eval` (+ − × ÷ ^, parentheses, percent); doc search through `retrieve_context`; web search hook for T-046.
- [x] Tool results go into the answer's context as untrusted `<tool_results>`; answers show **Tools used**.
- [x] Several Skills per session: the main Skill (dropdown, Switch Skill shortcut) plus up to two more (`session_skills`, migration 0006); prompts combined, tools pooled, cue unless all are full-only.
**Tests**
- [x] Rust: extra Skills stored in order, main and repeats dropped, limit of 3, unknown Skill refused, moving the main Skill, deleted Skill drops out; mock model's agent step.
- [x] Unit: calculator (precedence, percent, powers, refuses non-arithmetic and division by zero); step parsing; tool loop with allowlist, call/time/repeat limits, failures, web search hook; combined Skills; router passes tool results and answers from them.
- [x] Component: Tools used list and "checking" state; More Skills picker (main excluded, limit, moving the main Skill).
- [x] Playwright (`tests/ui/agents.spec.ts`): Negotiation Coach uses the calculator; two Skills at once; axe.
- [x] E2E (`tests/e2e/specs/agents.e2e.ts`): calculator step through the real gateway; extra Skill saved.
- [ ] Manual 🖥️: with a real model, tools are used when they help and not otherwise (T-039); web search after T-046.

### T-046 Web search provider and per-session overrides

**Refs:** PRD KEY-8, KEY-9; T-003 (P2); TECHNICAL.md 6.5 (`web_search`, `create_session`, `session_provider_options`), 7.1 (`gateway`)
**Scope**
- [x] Web search adapters for Tavily and Brave in the gateway, used by the Settings key check and by `web_search` (the provider chosen under Web search in Settings; off → clear error). At most 5 results, http(s) links only, short plain snippets.
- [x] Agent Skills' `web_search` tool uses it (T-045 hook).
- [x] Per-session providers: **Options** next to **New session** picks another speech-to-text or answer provider for that session only; ready providers only (key saved or none needed), others shown but disabled.
- [x] The session keeps its providers: start uses its speech-to-text provider; answers, notes, "What did I miss?" and the brief use its answer provider (Settings model when it's the Settings provider, else that provider's defaults).
**Tests**
- [x] Rust: Tavily and Brave over a mock HTTP server (keys, query, parsing), error mapping, missing key, unknown provider, empty query, mock mode; result parsing limits; an override must be known, implemented and have its key; a session keeps its providers; mock model's web search step.
- [x] Unit: `sessionPlan`; answers use the session's provider; an agent's web search goes through the router.
- [x] Component: Options picker with defaults, creating a session with overrides, defaults without options.
- [x] Playwright (`tests/ui/web-search-overrides.spec.ts`): session with its own providers, agent web search shown under Tools used, axe.
- [x] E2E (`tests/e2e/specs/web-search-overrides.e2e.ts`): providers stored by Rust; web search through the real gateway (mock mode).
- [ ] Manual 🖥️: real Tavily and Brave keys (key check and results); a session on OpenAI while Settings says Anthropic. Speech-to-text overrides offer only providers marked implemented in the catalog (Deepgram in this build; AssemblyAI and Soniox once T-021's live checks pass).

### T-047 Custom vocabulary

**Refs:** PRD LIS-5 (with LIS-5a); TECHNICAL.md 7.1 (`settings`, `stt`)
**Scope**
- [x] Settings → Custom vocabulary: product names, acronyms and people, one per line or comma-separated; up to 100 terms of up to 50 characters; trimmed and de-duplicated; a live count.
- [x] When a session starts, keyterms = the user's names (LIS-5a) then the vocabulary, without duplicates.
- [x] Every speech-to-text provider gets them, at most 100: Deepgram `keyterm`, AssemblyAI `keyterms_prompt`, Soniox `context.terms`, local Whisper's prompt (cut to 600 characters).
**Tests**
- [x] Rust: vocabulary cleaned, limited, saved and merged after the names; each provider's request carries the terms up to the limit (names first); Whisper prompt stays in its window; mock provider applies the spellings.
- [x] Component: save with cleaning and count, limit error, clear, axe.
- [x] Playwright (`tests/ui/vocabulary.spec.ts`): save, count, validation, axe.
- [x] E2E (`tests/e2e/specs/vocabulary.e2e.ts`): vocabulary saved in Settings changes the live transcript's spelling through the real speech-to-text pipeline (mock provider).
- [ ] Manual 🖥️: accuracy gain on real audio with Deepgram, AssemblyAI and Soniox (T-021, T-027).

### T-048 Click-through mode and themes

**Refs:** PRD UI-5, UI-8; TECHNICAL.md 6.5 (`set_click_through`), 7.2 (themes), 12.3 (shortcuts)
**Scope**
- [x] Click-through: View → Click-through makes the overlay ignore the mouse (`set_ignore_cursor_events`) while staying visible; a notice shows the shortcut to turn it off; never saved.
- [x] New global shortcut Click-through on/off (Ctrl/Cmd+Shift+T). Saved shortcut sets from older versions get the new default added without losing custom keys (`complete`).
- [x] Themes: Dark, Light, High contrast and System (follows the OS) in Settings → General; both windows switch live (`settings-changed` event); the glass panel has light and high-contrast variants (high contrast is solid).
- [x] Light-theme fix: Save buttons that used the accent background with secondary text now use `btn-primary`.
**Tests**
- [x] Rust: Click-through action and label, `complete` adds missing defaults but keeps custom keys and refuses conflicts; theme defaults to dark, saves, rejects unknown values.
- [x] Unit: theme resolution (System follows the OS), `data-theme`, following settings changes from another window.
- [x] Component: Click-through switch → command and notice, shortcut turns it off, events from elsewhere; Settings Theme select and the new shortcut row.
- [x] Playwright (`tests/ui/themes.spec.ts`): axe with contrast on every overlay tab and the Settings window in Light, High contrast and Dark; click-through on and off.
- [x] E2E (`tests/e2e/specs/themes-click-through.e2e.ts`): a Settings theme reaches the overlay through Rust; click-through through the real window command and event.
- [ ] Manual 🖥️: clicks really pass through to the app behind on Windows and macOS, and the shortcut brings the overlay back.

### T-049 Glass overlay: translucent background, crisp text

**Refs:** PRD UI-1, UI-3, Accessibility (NFR table); TECHNICAL.md 7.2
**Scope**
- [x] The panel background is a dark translucent tint, so the screen behind (the meeting video, slides) stays visible through it.
- [x] The opacity setting (20% to 100%) controls only the background tint. Text, buttons and icons stay fully opaque; today the whole panel fades.
- [x] Raised surfaces (cards, inputs, chips, the session bar) are translucent layers on the glass, not opaque blocks, so the backdrop shows through the whole panel.
- [x] Menus and dialogs (View menu, capture menu, region picker, consent dialog) stay solid so they are always readable.
- [x] Text stays readable over bright or busy content: a soft text shadow that strengthens as the tint gets lighter, and hairline borders on the glass.
- [x] Settings window unchanged (opaque). New installs default to 80% so the glass is visible out of the box (was 90%); saved settings are kept.
- [x] No native blur (macOS vibrancy, Windows acrylic) for now: it hides what is behind the panel, which is the point of this task, and acrylic lags while dragging on Windows 10.
**Tests**
- [x] Component: the opacity setting sets the background tint, and the panel itself is never faded (no CSS `opacity` on the root).
- [x] Component: the slider is labelled "Background opacity".
- [x] Playwright: overlay over a busy backdrop image at 40% and 100%; axe on every tab still passes.
- [x] E2E: suite passes unchanged (macOS 26.7 local run, 12 specs; CI also runs it).
- [ ] Manual 🖥️: over a live video call on Windows and macOS, the call is visible behind the panel and answers stay readable at the default opacity.

## Phase 4 details

### T-051 macOS device verification 🖥️

**Refs:** [phase-0-manual-checks.md](phase-0-manual-checks.md), [phase-1-manual-checks.md](phase-1-manual-checks.md)
**Scope:** Test every in-progress task on macOS and close it; move the Windows half to Phase 5 and live-key checks to T-039.
**Environment:** macOS 26.7, Apple Silicon (Mac17,8, 48 GB, one built-in display), Node 22.23, Rust 1.97.0, Playwright WebKit 26.6 (the engine of the macOS Tauri webview) and Chromium. No provider keys, no Ollama, no second display.

**Tests run and passed (2026-11-09)** · Report: [results](Macos_testing_report.md#results), [changes made while testing](Macos_testing_report.md#changes-made-while-testing), [per-task evidence](Macos_testing_report.md#tasks-closed-after-macos-testing), [real-app E2E](Macos_testing_report.md#real-app-end-to-end-on-macos), [all 53 screenshots](Macos_testing_report.md#all-53-playwright-tests-in-webkit)
- [x] Playwright UI suite in WebKit: 53/53, one screenshot per test. Same suite in Chromium: 53/53. `code-copy.spec.ts` now stubs the clipboard on WebKit (Playwright has no clipboard permission there) and keeps the real clipboard on Chromium.
- [x] 15 new Playwright specs: `library` (T-019, T-017), `overlay-controls` (T-029), `protection` (T-011), `diagnostics` (T-012). They found and fixed a mock event-bus bug that dropped listeners under React StrictMode (`src/ipc/sessionEvents.ts`; browser mock only).
- [x] Vitest: 277/277. `pnpm lint` and `pnpm typecheck`: clean.
- [x] `cargo test --workspace`: 292 passed, 0 failed.
- [x] WebdriverIO E2E on the real app (WebKit, mock providers, fixture audio): 13 tests in 12 specs passed; log scan found no keys or meeting content.
- [x] Device tests: real keychain round trip; microphone frames; system-audio frames with audio playing (0.32 s). In silence the system-audio test fails after its 3 s timeout, which matches the check's "play any audio" precondition. The first run hung over 60 s, most likely on the one-time System Audio Recording prompt; it hasn't recurred.

**Not run on macOS, accepted at closure** (each task's section in the [report](Macos_testing_report.md#tasks-closed-after-macos-testing) has the same line)

| Task | Check not run | Now tracked in |
| --- | --- | --- |
| [T-008](Macos_testing_report.md#t-008-settings-window-providers-and-api-keys) | Save with a real Anthropic key; keychain "Always Allow" prompt | (accepted; keychain round trip passed) |
| [T-009](Macos_testing_report.md#t-009-audio-capture-microphone-and-system-audio) | Live-call glitch check; denied-permission message | (accepted) |
| [T-010](Macos_testing_report.md#t-010-deepgram-streaming) | Live Deepgram latency | T-039 |
| [T-011](Macos_testing_report.md#t-011-capture-exclusion-and-protection-status) | Hiding in live meeting apps with a second viewer; ADR-015 titles | (accepted on macOS) |
| [T-012](Macos_testing_report.md#t-012-latency-metrics-and-baselines) | Baselines on the 8 GB MacBook Air | T-038 |
| [T-015](Macos_testing_report.md#t-015-global-shortcuts) | Shortcuts while a meeting app has focus | (accepted on macOS) |
| [T-017](Macos_testing_report.md#t-017-ingest-parse-chunk-embed) | Image files through a vision model | T-039 |
| [T-019](Macos_testing_report.md#t-019-library-documents-in-a-session) | Native picker and Finder drag | (accepted; Playwright covers the flows) |
| [T-021](Macos_testing_report.md#t-021-prompts-skills-two-tier-answers), [T-041](Macos_testing_report.md#t-041-meeting-memory-agenda-recap-action-items) to [T-047](Macos_testing_report.md#t-047-custom-vocabulary) | Real-model quality, live Tavily/Brave, live vocabulary accuracy | T-039 |
| [T-025](Macos_testing_report.md#t-025-screenshots-and-answer-from-screen) | Capture during a live call; Screen Recording prompt | (accepted on macOS) |
| [T-027](Macos_testing_report.md#t-027-stt-providers-assemblyai-soniox-local-whisper) | Live AssemblyAI/Soniox, Whisper accuracy, Hinglish | T-039 |
| [T-029](Macos_testing_report.md#t-029-overlay-controls-and-private-display) | Two-monitor restore | (accepted on macOS) |
| [T-048](Macos_testing_report.md#t-048-click-through-mode-and-themes) | Physical clicks through to another app | (accepted on macOS) |
| [T-049](Macos_testing_report.md#t-049-glass-overlay) | Look over a live call | (accepted on macOS) |

## Phase 5 details

Tick each box with the Windows build, meeting app and result. Record screen-share results in [TECHNICAL.md 8.5](TECHNICAL.md#85-known-issues-and-limitations) and baselines in [TECHNICAL.md 13.2](TECHNICAL.md#132-benchmarks-and-baselines). Differences from macOS are expected for window exclusion, system audio and click-through; note them in the box.

### T-050 Windows setup and automated suite 🖥️

Prerequisites in [TECHNICAL.md 6.1](TECHNICAL.md#61-prerequisites) (Rust via rustup, Node 22+ with pnpm, Microsoft C++ Build Tools).

```powershell
git clone https://github.com/coderArchitech/Sidekick.git
cd Sidekick
pnpm install
pnpm verify
pnpm build:e2e
pnpm test:e2e
```

- [ ] Windows 10: `pnpm verify` and `pnpm test:e2e` green.
- [ ] Windows 11: `pnpm verify` and `pnpm test:e2e` green.

### T-052 to T-063 Windows checks 🖥️

- [ ] T-052 (T-008): key saved, survives restart, removed from Credential Manager. Steps: [phase-0, T-008](phase-0-manual-checks.md#t-008-settings-window-in-the-real-app-5-minutes).
- [ ] T-053 (T-009): `cargo test -p sidekick-audio -- --ignored` passes with audio playing, and again during a real call with no glitches for the other participants. Steps: [phase-0, T-009](phase-0-manual-checks.md#t-009-audio-capture-10-minutes).
- [ ] T-054 (T-010): Deepgram p50 under 1,000 ms and p95 under 1,500 ms (NFR-1). Steps: [phase-0, T-010](phase-0-manual-checks.md#t-010-deepgram-latency-10-minutes).
- [ ] T-055 (T-011): hidden from the viewer in Teams, Zoom, Meet, Webex and Slack huddles (full screen and window); share-window titles recorded for ADR-015. Steps: [phase-0, T-011](phase-0-manual-checks.md#t-011-screen-share-hiding-20-minutes-per-os).
- [ ] T-056 (T-012): cold start, `.msi` size, idle CPU and RAM on the 8 GB reference laptop. Steps: [phase-0, T-012](phase-0-manual-checks.md#t-012-baselines-15-minutes).
- [ ] T-057 (T-015): every shortcut acts in Sidekick while a meeting app has focus, and the app doesn't react. Steps: [phase-1, T-015](phase-1-manual-checks.md#t-015-shortcuts-while-a-meeting-app-has-focus-5-minutes-per-os).
- [ ] T-058 (T-019): native picker and Explorer drag-and-drop (steps below).
- [ ] T-059 (T-025): capture during a live call shows no notification; window and area capture work. Steps: [phase-1, T-025](phase-1-manual-checks.md#t-025-screenshots-during-a-live-call-10-minutes-per-os).
- [ ] T-060 (T-027): AssemblyAI, Soniox and local Whisper (models under `%APPDATA%\com.sidekick.app\models`). Steps: [phase-1, T-027](phase-1-manual-checks.md#t-027-speech-to-text-providers-30-minutes).
- [ ] T-061 (T-029): overlay position restored across restarts on two monitors; falls back when one is unplugged. Steps: [phase-1, T-029](phase-1-manual-checks.md#t-029-overlay-position-on-two-monitors-10-minutes-per-os).
- [ ] T-062 (T-048): clicks pass through to the app behind; the shortcut brings the overlay back (steps below).
- [ ] T-063 (T-049): the call is visible behind the glass and answers stay readable at 80% (steps below).

**Steps for checks that had none**

*T-058, native picker and drag-and-drop (5 minutes)*
1. Start a session and open **Documents**.
2. Click **Add files**, pick a PDF and a `.docx` in the Windows picker. Both reach **Ready**.
3. Drag a `.pptx` from Explorer onto the overlay. The Documents panel opens by itself and the file reaches **Ready**.
4. Drag a `.png` or `.exe`. Sidekick refuses it with the list of supported types.

*T-062, click-through (5 minutes)*
1. Put a browser window behind the overlay. Turn on **View → Click-through**.
2. Click a link in the browser through the overlay. The click reaches the browser.
3. Press the click-through shortcut (shown in the View menu). The overlay takes clicks again.

*T-063, glass over a live call (5 minutes)*
1. Join a video call with the default opacity (80%).
2. The participants' video is visible behind the overlay and answer text is readable over bright and dark scenes.
3. Move **View → Background** between 20% and 100%. Text never fades, only the tint.
