#!/usr/bin/env node
// Runs the E2E suite (T-033): the main specs on one app, then the LLM-outage
// spec on a fresh app with a failing model. Extra args go to wdio
// (e.g. `pnpm test:e2e --spec tests/e2e/specs/session.e2e.ts`).
import { spawnSync } from "node:child_process";

const extra = process.argv.slice(2);
const only = extra.find((a) => a.includes("llm-failure"));
const runs = only ? [{ E2E_LLM_FAILS: "1" }] : [{}, { E2E_LLM_FAILS: "1" }];
let failed = false;
for (const env of runs) {
  const args = [
    "exec",
    "wdio",
    "run",
    "tests/e2e/wdio.conf.ts",
    ...(env.E2E_LLM_FAILS ? [] : extra),
  ];
  const r = spawnSync("pnpm", args, {
    stdio: "inherit",
    env: { ...process.env, ...env },
    shell: process.platform === "win32",
  });
  if (r.status !== 0) failed = true;
  if (extra.length && !only) break; // a specific spec was asked for
}
process.exit(failed ? 1 : 0);
