# End-to-end tests (WebdriverIO)

Full-app tests (T-033, tech doc 8.1, ADR-010): the real Rust core and UI, with mock providers, fixture audio and a scripted mock transcript. Run them on Windows, macOS or Linux:

```bash
pnpm build:e2e
```

```bash
pnpm test:e2e
```

On Linux without a display, prefix the second command with `xvfb-run -a`.

## How it works

- `pnpm build:e2e` makes a **debug** build with the `e2e` cargo feature and `src-tauri/tauri.e2e.conf.json`. Only that build contains `tauri-plugin-wdio` and `tauri-plugin-wdio-webdriver` (a WebDriver server inside the app, so it works on macOS too), their permissions, `withGlobalTauri`, and the `@wdio/tauri-plugin` frontend import. Normal and release builds contain none of it.
- `wdio.conf.ts` starts the app once per run with a fresh data folder and these variables: `SIDEKICK_MOCK_PROVIDERS=1`, `SIDEKICK_AUDIO_FIXTURE` (a generated WAV with one tone burst per utterance), and `SIDEKICK_MOCK_TRANSCRIPT` (what the mock speech-to-text "hears" for each burst; see `fixtures.ts`).
- `scripts/e2e.mjs` runs two passes: the main specs on one app, then `llm-failure.e2e.ts` on a fresh app started with `SIDEKICK_MOCK_LLM_FAIL=1`.
- After each pass, `scripts/scan-logs.mjs` checks the app's logs for key-shaped strings and fixture content and fails the run if it finds any.
- Failing tests save a screenshot in `test-results/e2e-screens/`.

## Writing specs

Specs share one app, in order. Start with `createAndStart()` (it ends a session left live by an earlier spec). Prefer accessible names (`aria/…`) and exact text (`button=…`). To wait for text, use `waitForText()`: under WebKitGTK, partial-text selectors (`*=…`) resolve to elements reported as not displayed.
