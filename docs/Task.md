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
5. Tasks marked 🖥️ also pass their manual check on a real Windows and macOS machine. Until then they stay at "In progress: testing" with a note.

**Test levels** used below: **Unit** (cargo test / Vitest), **Integration** (cargo test in `tests/`, mock HTTP with wiremock), **UI** (Playwright with mocked IPC), **E2E** (WebdriverIO on the real app with mock providers), **Manual** (checklist on real machines). See [TECHNICAL.md 8.1](TECHNICAL.md#81-test-strategy).

**Adding a task:** take the next free ID, add a board row and a detail section in the same phase. Don't renumber existing tasks.

## Board

### Phase 0: Foundation and risk retirement

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-001 | Project scaffold | ✅ Done | Branch `chore/scaffold` |
| T-002 | CI green on Windows and macOS | ⬜ To do | Needs a PR to trigger CI |
| T-003 | Apply doc fixes P1–P5 | ⛔ Blocked | Awaiting owner approval (TECHNICAL.md 14.6) |
| T-004 | `secrets` crate: OS keychain | ⬜ To do | |
| T-005 | `store` crate: encrypted database and migrations | ⬜ To do | Vector table waits on T-003 (P1) |
| T-006 | Settings storage and commands | ⬜ To do | |
| T-007 | `gateway` crate: LLM provider trait, mock and Anthropic | ⬜ To do | |
| T-008 | Settings window: providers and API keys | ⬜ To do | |
| T-009 | `audio` crate: microphone and system audio capture | ⬜ To do | 🖥️ |
| T-010 | `stt` crate: Deepgram streaming | ⬜ To do | |
| T-011 | `window` crate: capture exclusion and protection status | ⬜ To do | 🖥️ Highest technical risk |
| T-012 | `diagnostics` crate: latency metrics and baselines | ⬜ To do | |

### Phase 1: MVP (all P0 requirements)

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-013 | Sessions and consent | ⬜ To do | |
| T-014 | Live transcript tab | ⬜ To do | |
| T-015 | `hotkeys` crate: global shortcuts | ⬜ To do | 🖥️ |
| T-016 | Profile: name and nicknames | ⬜ To do | |
| T-017 | `ingest` crate: parse, chunk, embed | ⬜ To do | Embeddings need T-003 (P1) |
| T-018 | `retrieval` crate: hybrid search and context packs | ⬜ To do | |
| T-019 | Library feature: documents in a session | ⬜ To do | |
| T-020 | Orchestrator: question detection and modes | ⬜ To do | |
| T-021 | Orchestrator: prompts, Skills, two-tier answers | ⬜ To do | |
| T-022 | Live Answers tab | ⬜ To do | |
| T-023 | Chat tab | ⬜ To do | |
| T-024 | Rolling notes | ⬜ To do | |
| T-025 | `capture` crate: screenshots and answer from screen | ⬜ To do | 🖥️ |
| T-026 | More LLM providers: OpenAI, Gemini, Ollama | ⬜ To do | |
| T-027 | More STT providers: AssemblyAI, Soniox, local Whisper | ⬜ To do | |
| T-028 | `skills` crate and Skill builder | ⬜ To do | |
| T-029 | Overlay controls and private display | ⬜ To do | 🖥️ |
| T-030 | Data control: retention, delete session | ⬜ To do | |
| T-031 | Logging to file and log export | ⬜ To do | |
| T-032 | UI test suite (Playwright) | ⬜ To do | |
| T-033 | End-to-end test suite (WebdriverIO) | ⬜ To do | |
| T-034 | MVP acceptance pass | ⬜ To do | 🖥️ TECHNICAL.md 2.6 |

### Phase 2: V1 launch readiness

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-035 | Usage and cost meter | ⬜ To do | |
| T-036 | Backup and restore | ⬜ To do | |
| T-037 | Release pipeline: signing and auto-update | ⬜ To do | Needs signing certificates |
| T-038 | Performance soak and baselines | ⬜ To do | |
| T-039 | Answer quality eval | ⬜ To do | Needs real provider keys |
| T-040 | Release checklist and first beta | ⬜ To do | 🖥️ |

### Phase 3: P1 features (ship if time allows)

| ID | Task | Status | Notes |
| --- | --- | --- | --- |
| T-041 | Meeting memory extras: agenda, recap, action items | ⬜ To do | |
| T-042 | Answer follow-up chips | ⬜ To do | |
| T-043 | Screenshot quick actions and strip | ⬜ To do | |
| T-044 | Pre-meeting brief and Knowledge Packs | ⬜ To do | |
| T-045 | Agents (tools) and multiple active Skills | ⬜ To do | |
| T-046 | Web search provider and per-session overrides | ⬜ To do | |
| T-047 | Custom vocabulary | ⬜ To do | |
| T-048 | Click-through mode and themes | ⬜ To do | |

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
- [ ] Doc link check passes (all relative links and anchors resolve).
- [ ] Tests for each change are added in the task that implements it (T-005, T-007, T-010, T-017, T-021).

### T-004 `secrets` crate: OS keychain

**Refs:** KEY-5, KEY-6, KEY-7; TECHNICAL.md 3.2, 11.4, NFR-7; ADR-008
**Scope:** `crates/secrets`: `set`, `get`, `delete`, `status` for `sidekick/<provider>` entries; keys held as `SecretString`; `mask()` returns only the last 4 characters; a `KeyStore` trait so other crates and tests can use an in-memory store.

**Tests**
- [ ] Unit: `mask()` shows only the last 4 characters, handles keys shorter than 4.
- [ ] Unit: in-memory store set/get/delete/overwrite; multiple providers kept independently.
- [ ] Unit: `SecretString` values never appear in `Debug` output of any public type.
- [ ] Integration: real keychain round trip (ignored by default; run on CI for Windows and macOS).
- [ ] Unit: keychain errors map to `SidekickError` codes, message contains no key.

### T-005 `store` crate: encrypted database and migrations

**Refs:** TECHNICAL.md 4.1–4.6, NFR-8; ADR-004
**Scope:** `crates/store`: rusqlite with bundled SQLCipher; database key generated on first run and kept in the keychain via `secrets`; `rusqlite_migration` with `0001_init.sql`; WAL and foreign keys on; pre-migration backup (keep 3); repository functions per table as features need them; sqlite-vec loaded.

**Tests**
- [ ] Integration: fresh database migrates to the latest `user_version`.
- [ ] Integration: opening the database file without the key fails (NFR-8).
- [ ] Integration: a failing migration rolls back and keeps the backup; only 3 backups are kept.
- [ ] Integration: foreign keys enforced; deleting a session cascades to all child rows.
- [ ] Integration: sqlite-vec loads; insert and nearest-neighbour query return the expected row.
- [ ] Integration: FTS stays in sync with `chunks` on insert, update and delete (after T-003 P4).
- [ ] Unit: no API key is ever written to any table (scan after a test run).

### T-006 Settings storage and commands

**Refs:** KEY-1, KEY-6; TECHNICAL.md 4.5 (seed data), 5.1 (`get_settings`, `update_settings`)
**Scope:** Typed `Settings` struct with defaults (Deepgram, Auto-answer on, retention 30 days); stored in the `settings` table; `get_settings` and `update_settings(patch)` commands; settings window capability.

**Tests**
- [ ] Unit: defaults match TECHNICAL.md 4.5.
- [ ] Unit: partial patch updates only the given fields; unknown keys rejected.
- [ ] Integration: settings persist across store reopen.
- [ ] Unit: capability files grant settings commands to the Settings window only.

### T-007 `gateway` crate: LLM provider trait, mock and Anthropic

**Refs:** MOD-1, MOD-2, KEY-3, KEY-4; TECHNICAL.md 5.1 (`llm_stream`, `cancel_stream`, `list_models`, `set_api_key`, `test_api_key`, `delete_api_key`, `list_providers`), 5.2 (`answer.*` events), 5.5, 5.6
**Scope:** `LlmProvider` trait in `sidekick-core`; provider registry with `ProviderInfo`; mock provider (`SIDEKICK_MOCK_PROVIDERS=1`); Anthropic adapter with streaming; retries with backoff and `Retry-After`; cancellation; key commands that test then store keys.

**Tests**
- [ ] Unit: Anthropic request building (system prompt, messages, model, tools).
- [ ] Unit: SSE stream parsing into token deltas, usage and finish reason (recorded fixtures).
- [ ] Unit: error mapping 401/403 → `KEY_INVALID`, 429 → `RATE_LIMITED`, 5xx → `PROVIDER_UNAVAILABLE`.
- [ ] Integration (wiremock): retries 3 times with backoff, honours `Retry-After`, then fails.
- [ ] Integration: `cancel_stream` stops token events and returns `CANCELLED`.
- [ ] Integration: `set_api_key` stores only after a successful test; IPC responses never include the key.
- [ ] Unit: mock provider is deterministic.

### T-008 Settings window: providers and API keys

**Refs:** KEY-1 to KEY-7; TECHNICAL.md 3.7 (Settings window), 12.1
**Scope:** Second Tauri window; `settings` feature with Speech-to-text, LLM and Web search sections; provider and model dropdowns; masked key field with show/hide and paste; Test connection; Remove key; last 4 characters after save.

**Tests**
- [ ] Component: dropdowns list providers from `list_providers`; model dropdown follows provider.
- [ ] Component: key field masked by default, show/hide toggles, paste works.
- [ ] Component: Test connection shows success, `KEY_INVALID` and network errors distinctly.
- [ ] Component: after save only last 4 characters show; Remove key clears status.
- [ ] Component: switching active provider needs no re-entry when a key is saved.
- [ ] Accessibility (axe): no violations; full keyboard operation.

### T-009 `audio` crate: microphone and system audio capture 🖥️

**Refs:** LIS-1, LIS-4, LIS-6; TECHNICAL.md 3.2, 6.4 (`SIDEKICK_AUDIO_FIXTURE`), NFR-3, NFR-6; ADR-006
**Scope:** `AudioSource` trait; microphone via cpal; Windows system audio via WASAPI loopback; macOS system audio via ScreenCaptureKit; resample to 16 kHz mono PCM; Silero VAD; WAV fixture source; pause and resume; never writes audio to disk.

**Tests**
- [ ] Unit: resampler converts 48 kHz stereo to 16 kHz mono within tolerance.
- [ ] Unit: VAD marks speech and silence correctly on fixture audio.
- [ ] Unit: fixture source streams a WAV file in real time and in fast mode.
- [ ] Unit: pause stops frames; resume restarts without a gap error.
- [ ] Integration: no files are created in the data folder during capture (NFR-6).
- [ ] Manual (Windows, macOS): mic and system audio captured as separate streams during a Teams, Zoom and Meet call, with no dropouts in the meeting app.
- [ ] Manual (macOS): clear error and help link when Screen Recording permission is missing (`PERMISSION_DENIED`).

### T-010 `stt` crate: Deepgram streaming

**Refs:** LIS-2, LIS-3; KEY-2; TECHNICAL.md 5.2 (`transcript.*`, `stt.status`), 5.4 (`STT_DISCONNECTED`), NFR-1, NFR-4; ADR-005
**Scope:** `SttProvider` trait in `sidekick-core`; Deepgram Nova-3 WebSocket adapter with interim and final results, diarization for the system stream; mic stream labelled "Me"; reconnect within 3 s with 10 s audio buffer; mock provider replaying a fixture transcript.

**Tests**
- [ ] Unit: Deepgram message parsing into partial and final segments (recorded fixtures).
- [ ] Unit: mic segments labelled `me` / `is_user = 1`; system segments labelled `speaker_N`.
- [ ] Integration (mock WebSocket server): reconnects within 3 s and replays buffered audio; emits `stt.status` reconnecting → live.
- [ ] Integration: 401 maps to `KEY_INVALID` and stops reconnecting.
- [ ] Integration: fixture audio through the mock provider produces the expected transcript.
- [ ] Manual: transcript lag measured against NFR-1 on a real call; result recorded in TECHNICAL.md 13.2.

### T-011 `window` crate: capture exclusion and protection status 🖥️

**Refs:** UI-4, UI-4a, UI-4b; TECHNICAL.md 5.1 (`get_capture_protection`), 5.2 (`capture.protection`, `screen_share.detected`), NFR-9; ADR-007
**Scope:** Windows `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)`; macOS window sharing type none; status check per OS version; auto-hide fallback when screen sharing is detected and exclusion isn't guaranteed; `SIDEKICK_DISABLE_EXCLUSION`.

**Tests**
- [ ] Unit: status logic returns protected / not guaranteed / unavailable per OS and version.
- [ ] Unit: fallback hides the overlay when sharing starts and status isn't protected; restores after.
- [ ] Unit: `SIDEKICK_DISABLE_EXCLUSION=1` turns exclusion off and status says so.
- [ ] Manual (Windows 10, 11; macOS 14, 15, latest): overlay not visible to others when sharing full screen and a window in Teams, Zoom, Meet, Webex, Slack huddles; results recorded in a table in TECHNICAL.md 8.5.

### T-012 `diagnostics` crate: latency metrics and baselines

**Refs:** TECHNICAL.md 10.1, 13.1, 13.2; NFR-1, NFR-2
**Scope:** Record per-stage timings (end of speech, segment, detection, retrieval, first token); p50/p95 over a rolling window; `get_diagnostics` command; Diagnostics panel in Settings.

**Tests**
- [ ] Unit: percentile calculation on known samples.
- [ ] Unit: rolling window drops old samples.
- [ ] Component: Diagnostics panel shows values and healthy/unhealthy state.
- [ ] Manual: Phase 0 baselines recorded in TECHNICAL.md 13.2.

## Phase 1 details

### T-013 Sessions and consent

**Refs:** CTX-1; PRD 8; TECHNICAL.md 3.3 (session start), 5.1 (`create_session`, `confirm_consent`, `start_session`, `pause_session`, `resume_session`, `end_session`, `delete_session`, `list_sessions`), 5.4 (`CONSENT_REQUIRED`)
**Scope:** `session` and `consent` crates; lifecycle draft → live → paused → ended; consent dialog with "Participants agreed" and "Transcribe me only"; me-only mode starts only the mic stream; consent log kept 1 year.

**Tests**
- [ ] Unit: allowed and rejected lifecycle transitions.
- [ ] Unit: `start_session` without consent returns `CONSENT_REQUIRED`.
- [ ] Unit: me-only consent starts mic capture only; system audio never starts.
- [ ] Integration: consent log row written with date and option; survives session deletion rules (1 year).
- [ ] Integration: `list_sessions` pagination (cursor, limit default 50, max 200).
- [ ] Component: consent dialog blocks Start until an option is chosen.
- [ ] E2E: create session → consent → start with fixture audio → transcript appears → end.

### T-014 Live transcript tab

**Refs:** LIS-3, LIS-6; TECHNICAL.md 3.2 (`transcript`), 5.2
**Scope:** Transcript feature subscribing to `transcript.partial` and `transcript.segment`; speaker names with rename and merge; partial text replaced by final; listening indicator; bookmarks list.

**Tests**
- [ ] Component: partial text replaced by final segment.
- [ ] Component: renaming a speaker updates all their segments.
- [ ] Component: listening indicator reflects `session.state` and pause.
- [ ] Unit: token batching every 50 ms (TECHNICAL.md 13.5) keeps render count bounded.

### T-015 `hotkeys` crate: global shortcuts 🖥️

**Refs:** UI-6, LIS-6, QA-3; TECHNICAL.md 12.3
**Scope:** Register shortcuts from settings; emit `hotkey.pressed`; conflicts reported; customisable in Settings → Shortcuts.

**Tests**
- [ ] Unit: default bindings match TECHNICAL.md 12.3 per OS.
- [ ] Unit: conflicting or invalid bindings rejected with a clear error.
- [ ] Component: each action handler (show/hide, ask, screenshot, Auto, pause, switch Skill, bookmark) runs on its event.
- [ ] Manual: shortcuts work while a meeting app has focus on Windows and macOS.

### T-016 Profile: name and nicknames

**Refs:** QA-2; TECHNICAL.md 12.4; T-003 (P5)
**Scope:** Settings → Profile; names and nicknames stored in settings; passed to STT as vocabulary boost and to detection.

**Tests**
- [ ] Component: add, edit and remove nicknames.
- [ ] Unit: STT config includes the names as keywords.

### T-017 `ingest` crate: parse, chunk, embed

**Refs:** CTX-2, CTX-3; TECHNICAL.md 3.3 (document upload), 5.1 (`ingest_document`, `delete_document`), 5.2 (`ingest.progress`), 13.5
**Scope:** Parsers for PDF, DOCX, PPTX, XLSX, TXT, MD, images (OCR or vision description), URLs; ~500-token chunks with 50-token overlap and page numbers; duplicate detection by SHA-256; batched embeddings (100 per request, 3 docs in parallel); progress events; `EmbeddingProvider` from T-003.

**Tests**
- [ ] Unit: each parser extracts text and page numbers from fixture files.
- [ ] Unit: corrupted file returns `PARSE_FAILED`; unknown type returns `UNSUPPORTED_FILE`.
- [ ] Unit: chunker sizes, overlap and page attribution.
- [ ] Unit: same file twice is detected by hash and not re-embedded.
- [ ] Integration: fixture 50-page PDF ingests end to end in under 30 s with the mock embedder (CTX-3).
- [ ] Integration: progress events go queued → parsing → embedding → ready.
- [ ] Integration: deleting a document removes its chunks, vectors and FTS rows.

### T-018 `retrieval` crate: hybrid search and context packs

**Refs:** QA-4; TECHNICAL.md 5.1 (`retrieve_context`), 13.1 (100 ms budget)
**Scope:** Vector and FTS search filtered to the session, merged with reciprocal rank fusion; context pack = last 3 minutes of transcript + notes + top 5 chunks + latest screenshot; retrieval order transcript → documents → web.

**Tests**
- [ ] Unit: rank fusion ordering on known inputs.
- [ ] Unit: transcript window trimming to 3 minutes.
- [ ] Integration: golden queries return the expected chunk in the top 5 on fixture docs.
- [ ] Integration: results never include another session's documents.
- [ ] Benchmark: under 50 ms over 20k chunks (TECHNICAL.md 13.2).

### T-019 Library feature: documents in a session

**Refs:** CTX-1, CTX-2; TECHNICAL.md 12.2
**Scope:** Drag-and-drop and file picker; per-document status and errors; delete; URL input.

**Tests**
- [ ] Component: dropped files call `ingest_document`; progress events update status.
- [ ] Component: failed documents show the reason and a retry.
- [ ] UI: drag and drop with mocked IPC.

### T-020 Orchestrator: question detection and modes

**Refs:** QA-1, QA-2, QA-3; TECHNICAL.md 5.5 (600 ms debounce), 13.1
**Scope:** Rules for interrogatives and implicit asks; name and nickname matching (fuzzy, tolerant of STT spelling); optional small-model classifier; Auto vs Directed-only mode with header button and hotkey; manual asks in both modes; early detection on interim results for directed questions.

**Tests**
- [ ] Unit: labelled fixture set of 200+ utterances; precision ≥ 85%, recall ≥ 80% (PRD success metrics).
- [ ] Unit: name matching with misspellings and nicknames; no false trigger on similar words.
- [ ] Unit: Directed-only mode ignores undirected questions; manual asks always answered.
- [ ] Unit: debounce waits 600 ms after segment end and merges split questions.
- [ ] Component: Auto button and hotkey toggle mode and persist the session setting.

### T-021 Orchestrator: prompts, Skills, two-tier answers

**Refs:** QA-4, QA-5, QA-6; MOD-3; TECHNICAL.md 3.3, 5.5, 11.5 (prompt injection), T-003 (P3)
**Scope:** Prompt builder wrapping retrieved content as untrusted data; active Skill instructions; fast cue model and main model in parallel; citation format; "not found in your docs" fallback; at most 2 questions in flight, newest cancels oldest.

**Tests**
- [ ] Unit: prompt snapshot tests per answer type (detected, chat, screenshot).
- [ ] Unit: retrieved text is fenced as untrusted data; injection strings in fixtures don't change instructions.
- [ ] Unit: citations parsed into `sources_json` (doc page, transcript time, URL).
- [ ] Unit: empty retrieval produces the not-found answer, never a guess.
- [ ] Unit: third concurrent question cancels the oldest; cancelled streams emit no further tokens.
- [ ] Unit: LLM failure leaves transcript and notes running (NFR-5).

### T-022 Live Answers tab

**Refs:** QA-5, QA-6, QA-7; TECHNICAL.md 3.2 (`answers`), 11.5
**Scope:** Cue line, expandable full answer, sources list, code blocks with Shiki highlighting and copy, thumbs up/down (`rate_answer`), sanitised Markdown with no raw HTML.

**Tests**
- [ ] Component: cue shows first, expands to full answer, then sources.
- [ ] Component: copy button copies code only when clicked (UI-4c).
- [ ] Component: raw HTML and script in Markdown are not rendered.
- [ ] Component: rating calls `rate_answer` with the right value.
- [ ] UI: streamed tokens render progressively with mocked events.

### T-023 Chat tab

**Refs:** QA-8; TECHNICAL.md 12.2
**Scope:** Typed questions with full session context; follow-ups with history; per-session chat history; Ask hotkey focuses the input.

**Tests**
- [ ] Component: sending a message streams an answer and keeps history.
- [ ] Component: follow-up includes previous turns in the request.
- [ ] Integration: chat questions stored with `source = chat`.
- [ ] UI: Ask hotkey focuses the chat input.

### T-024 Rolling notes

**Refs:** MEM-1; TECHNICAL.md 3.3 (rolling notes), 5.1 (`save_notes`)
**Scope:** Every 45 s, send new transcript plus current notes to the fast model; save with optimistic version check; Notes tab shows topics, decisions, open questions; summary on session end.

**Tests**
- [ ] Unit: job runs every 45 s only while live and only with new transcript.
- [ ] Unit: version conflict on `save_notes` retries with the latest notes.
- [ ] Component: Notes tab renders sections and updates live.
- [ ] Integration: `end_session` produces a summary with decisions and action items (mock model).

### T-025 `capture` crate: screenshots and answer from screen 🖥️

**Refs:** SCR-1 to SCR-4; UI-4c; TECHNICAL.md 5.1 (`capture_screenshot`), 5.2 (`screenshot.captured`), 13.5
**Scope:** Region, window and full-screen capture; downscale to 1568 px long side; encrypted BLOB storage; capture-and-ask; latest screenshot added to context; never copied to clipboard or shared folders.

**Tests**
- [ ] Unit: downscale keeps aspect ratio and the 1568 px limit.
- [ ] Integration: screenshot stored encrypted and linked to the session.
- [ ] Unit: screenshot questions include the image and cite it as a source.
- [ ] Component: capture-and-ask flow (capture, type question, answer).
- [ ] Manual: capture during a live call doesn't notify or interrupt the meeting app.

### T-026 More LLM providers: OpenAI, Gemini, Ollama

**Refs:** MOD-1, KEY-3; TECHNICAL.md 3.6, 7.5
**Scope:** Adapters following TECHNICAL.md 7.5, including vision and `list_models`; Ollama without a key on localhost.

**Tests**
- [ ] Unit (each provider): request building, stream parsing, error mapping from recorded fixtures.
- [ ] Integration (wiremock, each provider): happy path, 401, 429, 500.
- [ ] Unit: Ollama unreachable maps to `PROVIDER_UNAVAILABLE` with a helpful message.

### T-027 More STT providers: AssemblyAI, Soniox, local Whisper

**Refs:** KEY-2; LIS-2; TECHNICAL.md 3.6; PRD 11 (Hinglish evaluation)
**Scope:** Adapters behind `SttProvider`; whisper-rs in `spawn_blocking` behind the `local-whisper` feature flag; model download with hash check.

**Tests**
- [ ] Unit (each provider): message parsing and error mapping from fixtures.
- [ ] Integration: Whisper transcribes the short English fixture within tolerance.
- [ ] Unit: model download rejects a file with the wrong SHA-256.
- [ ] Manual: Hinglish comparison across providers recorded (PRD open question).

### T-028 `skills` crate and Skill builder

**Refs:** SKL-1, SKL-2, SKL-3; TECHNICAL.md 5.7, 11.5 (malicious Skill); ADR-009
**Scope:** Load and validate Skill files against `schemas/skill.schema.json`; save, delete, import, export; built-in templates seeded on first run; no-code builder UI; import shows requested tools for approval; tool allowlist.

**Tests**
- [ ] Unit: valid templates pass schema validation; invalid files give field-level errors.
- [ ] Unit: unknown tools are rejected; import lists tools for approval.
- [ ] Unit: export then import round trip is lossless.
- [ ] Golden: each built-in template has one golden-answer case (TECHNICAL.md 7.5).
- [ ] Component: builder creates a Skill from a template and saves it.

### T-029 Overlay controls and private display 🖥️

**Refs:** UI-1 to UI-4c, UI-7; TECHNICAL.md 12.2
**Scope:** Opacity slider (done in T-001), resize, drag, snap to edges, remember position per monitor, font size, compact/expanded, shield indicator wired to T-011, show/hide.

**Tests**
- [ ] Unit: snapping and per-monitor position persistence logic.
- [ ] Component: font size and compact mode change layout; shield colour follows protection status.
- [ ] Accessibility (axe): overlay has no violations.
- [ ] Manual: position restored across restarts on multi-monitor setups.

### T-030 Data control: retention, delete session

**Refs:** TECHNICAL.md 4.4, 10.7; PRD 6 (data control)
**Scope:** Retention job at startup and every 6 hours; per-session retention setting; delete everything button; VACUUM when idle; consent log kept 1 year.

**Tests**
- [ ] Unit: retention cut-off per option (session, 7d, 30d, forever).
- [ ] Integration: expired sessions deleted with all child rows; consent log kept.
- [ ] Integration: after delete and VACUUM, deleted text isn't found in the database file.

### T-031 Logging to file and log export

**Refs:** TECHNICAL.md 7.7, 10.3, NFR-7
**Scope:** JSON-line logs with daily rotation (14 days, 50 MB); frontend `log` command; correlation IDs; Export logs zip with diagnostics snapshot.

**Tests**
- [ ] Unit: rotation and size cap.
- [ ] Integration: log scan finds no key patterns or fixture phrases after a full E2E run.
- [ ] Integration: export zip contains logs and diagnostics, no database.

### T-032 UI test suite (Playwright)

**Refs:** TECHNICAL.md 8.1, ADR-010
**Scope:** `tests/ui/` with mocked IPC events; flows: Auto-answer toggle, Chat, code copy, settings dropdowns, consent dialog; axe checks; added to CI.

**Tests**
- [ ] UI: each flow above has a spec; axe passes on every screen.
- [ ] CI: UI job green.

### T-033 End-to-end test suite (WebdriverIO)

**Refs:** TECHNICAL.md 8.1, 8.3, ADR-010
**Scope:** `tests/e2e/` with @wdio/tauri-service; mock providers and fixture audio; smoke suite on every PR (Windows, macOS), full suite nightly.

**Tests**
- [ ] E2E: session start with consent → fixture audio → transcript → detected question → cited answer.
- [ ] E2E: screenshot capture and ask.
- [ ] E2E: settings: add, test and remove a key (mock provider).
- [ ] E2E: LLM failure keeps transcript running (NFR-5).
- [ ] CI: smoke suite green on Windows and macOS.

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
