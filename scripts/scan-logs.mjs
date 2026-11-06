#!/usr/bin/env node
// Scans Sidekick log files for things that must never be logged (tech doc 7.7):
// key-shaped strings and phrases from the test fixtures (transcript, documents,
// answers). Run after the E2E suite (T-033) against its SIDEKICK_DATA_DIR.
//
//   node scripts/scan-logs.mjs <logs-dir> [extra phrase ...]
//
// Exits 1 and prints the file and line of each finding.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const KEY_PATTERNS = [
  /sk-(?:ant-|proj-)?[A-Za-z0-9_-]{16,}/,
  /AIza[0-9A-Za-z_-]{20,}/,
  /\b(?:bearer|token|basic)\s+[A-Za-z0-9._~+/=-]{12,}/i,
  /\b[0-9a-f]{32,}\b/i,
];

/** Content used by fixtures and mocks that must stay out of logs. */
export const FIXTURE_PHRASES = [
  "rate limit for the Pro tier",
  "600 requests per minute",
  "Mock answer to:",
  "Mock passage from",
  "zebra-quartz-falcon",
];

export function scan(dir, phrases = FIXTURE_PHRASES) {
  const findings = [];
  for (const name of readdirSync(dir)) {
    if (!name.endsWith(".log")) continue;
    const lines = readFileSync(join(dir, name), "utf8").split("\n");
    lines.forEach((line, i) => {
      for (const re of KEY_PATTERNS)
        if (re.test(line)) findings.push(`${name}:${i + 1}: key-shaped string`);
      for (const p of phrases)
        if (line.includes(p)) findings.push(`${name}:${i + 1}: content "${p}"`);
    });
  }
  return findings;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const [dir, ...extra] = process.argv.slice(2);
  if (!dir) {
    console.error("usage: node scripts/scan-logs.mjs <logs-dir> [extra phrase ...]");
    process.exit(2);
  }
  const findings = scan(dir, [...FIXTURE_PHRASES, ...extra]);
  if (findings.length) {
    console.error(findings.join("\n"));
    process.exit(1);
  }
  console.log(`No keys or fixture content in ${dir}.`);
}
