import { mkdtempSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { describe, expect, it } from "vitest";
import { scan } from "./scan-logs.mjs";

describe("scan-logs", () => {
  it("flags keys and fixture content, and passes clean logs", () => {
    const dir = mkdtempSync(join(tmpdir(), "logs-"));
    writeFileSync(
      join(dir, "sidekick-2026-11-06.log"),
      '{"message":"first token","request_id":"a1:cue"}\n',
    );
    expect(scan(dir)).toEqual([]);
    writeFileSync(
      join(dir, "sidekick-2026-11-07.log"),
      '{"message":"x sk-ant-api03-AAAAAAAAAAAAAAAAAAAA"}\n{"message":"heard: 600 requests per minute"}\n',
    );
    expect(scan(dir)).toEqual([
      "sidekick-2026-11-07.log:1: key-shaped string",
      'sidekick-2026-11-07.log:2: content "600 requests per minute"',
    ]);
  });
});
