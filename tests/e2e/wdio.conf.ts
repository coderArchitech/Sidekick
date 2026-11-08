import { mkdirSync, mkdtempSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { scan } from "../../scripts/scan-logs.mjs";
import { prepareRun } from "./fixtures";

// Full-app tests (tech doc 8.1, ADR-010): the real Rust core with mock
// providers and fixture audio, driven through the embedded WebDriver of an
// E2E build (`pnpm build:e2e`, cargo feature `e2e`). See README.md here.
//
// The service starts one app for the run (before any spec), so specs share
// it and run in order. `E2E_LLM_FAILS=1` (wdio.failure.conf.ts) starts it
// with a failing model instead.
const root = mkdtempSync(join(tmpdir(), "sidekick-e2e-"));
const llmFails = process.env.E2E_LLM_FAILS === "1";
const results = join(import.meta.dirname, "../../test-results");
mkdirSync(join(results, "e2e-screens"), { recursive: true });

const binary = join(
  import.meta.dirname,
  "../../target/debug",
  process.platform === "win32" ? "sidekick-app.exe" : "sidekick-app",
);

const serviceOptions = {
  env: prepareRun(root, { llmFails }),
  captureBackendLogs: true,
  logDir: join(results, "e2e-logs"),
};

export const config: WebdriverIO.Config = {
  runner: "local",
  specs: llmFails
    ? ["./specs/llm-failure.e2e.ts"]
    : [
        "./specs/session.e2e.ts",
        "./specs/meeting-memory.e2e.ts",
        "./specs/follow-ups.e2e.ts",
        "./specs/screenshot.e2e.ts",
        "./specs/screenshot-actions.e2e.ts",
        "./specs/knowledge-packs.e2e.ts",
        "./specs/agents.e2e.ts",
        "./specs/web-search-overrides.e2e.ts",
        "./specs/settings.e2e.ts",
        "./specs/vocabulary.e2e.ts",
        "./specs/themes-click-through.e2e.ts",
      ],
  maxInstances: 1,
  // Typed loosely: the service reads these keys, its published types lag behind.
  capabilities: [
    {
      browserName: "tauri",
      "tauri:options": { application: binary },
      // `env` is read by the service's embedded launcher but missing from its types.
      "wdio:tauriServiceOptions": serviceOptions,
    } as unknown as WebdriverIO.Capabilities,
  ],
  services: [["@wdio/tauri-service", { driverProvider: "embedded", windowLabel: "overlay" }]],
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { timeout: 120_000 },
  waitforTimeout: 15_000,
  logLevel: "warn",
  /** Start every test on the overlay (the session may sit on the hidden Settings window). */
  async beforeTest() {
    await browser.tauri.switchWindow("overlay");
  },
  async afterTest(test, _context, { passed }) {
    if (!passed) {
      const name = `${test.parent} ${test.title}`.replace(/[^a-z0-9]+/gi, "-").slice(0, 80);
      await browser.saveScreenshot(join(results, "e2e-screens", `${name}.png`));
    }
  },
  /** After the run: no keys or meeting content in the logs (tech doc 7.7, T-031). */
  onComplete() {
    const findings: string[] = readdirSync(root).flatMap((run) => {
      try {
        return (scan(join(root, run, "data", "logs")) as string[]).map((f) => `${run}/${f}`);
      } catch {
        return [];
      }
    });
    if (findings.length) {
      console.error(`Log scan found content or keys:\n${findings.join("\n")}`);
      process.exitCode = 1;
    } else {
      console.log(`Log scan: no keys or fixture content in ${root}.`);
    }
  },
};
