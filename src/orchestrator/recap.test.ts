import { describe, expect, it, vi } from "vitest";
import type { LlmRequest, TranscriptLine } from "@/ipc";
import { mockAnswerBus } from "@/ipc/testing";
import { recap, recapWindow, RECAP_SYSTEM, type RecapDeps } from "./recap";

const line = (text: string, endMs: number, isUser = false): TranscriptLine => ({
  id: endMs,
  speakerLabel: "speaker_1",
  isUser,
  text,
  startMs: endMs - 1000,
  endMs,
});
const plan = { provider: "anthropic", full: "big", cue: "small" };

function deps(lines: TranscriptLine[]) {
  const requests: LlmRequest[] = [];
  const d: RecapDeps = {
    events: mockAnswerBus.events,
    llmStream: vi.fn((requestId: string, _p: string | null, req: LlmRequest) => {
      requests.push(req);
      queueMicrotask(() => {
        mockAnswerBus.emitToken({ requestId, delta: "- They chose the Pro tier\n" });
        mockAnswerBus.emitDone({
          requestId,
          usage: { inputTokens: 1, outputTokens: 1 },
          finishReason: "end_turn",
        });
      });
      return Promise.resolve({ status: "ok" as const, data: null });
    }),
    getTranscript: vi.fn(() =>
      Promise.resolve({ status: "ok" as const, data: { lines, bookmarks: [] } }),
    ),
  };
  return { d, requests };
}

describe("recap (PRD MEM-3)", () => {
  it("the window counts back from the newest line", () => {
    const lines = [line("old", 60_000), line("mid", 400_000), line("new", 600_000)];
    expect(recapWindow(lines, 5).map((l) => l.text)).toEqual(["mid", "new"]);
    expect(recapWindow(lines, 10).map((l) => l.text)).toEqual(["old", "mid", "new"]);
    expect(recapWindow([], 5)).toEqual([]);
  });

  it("sends only the window to the fast model and returns its bullets", async () => {
    const { d, requests } = deps([
      line("Long ago we said hello", 10_000),
      line("We picked the Pro tier", 500_000, true),
    ]);
    const text = await recap(d, "s1", 2, plan);
    expect(text).toBe("- They chose the Pro tier");
    expect(requests).toHaveLength(1);
    const content = requests[0]?.messages[0]?.content ?? "";
    expect(content).toContain("Me: We picked the Pro tier");
    expect(content).not.toContain("hello");
    expect(requests[0]?.model).toBe("small");
    expect(requests[0]?.system).toBe(RECAP_SYSTEM);
  });

  it("an empty meeting needs no model call", async () => {
    const { d, requests } = deps([]);
    expect(await recap(d, "s1", 5, plan)).toBe("Nothing was said in the last 5 minutes.");
    expect(requests).toHaveLength(0);
  });

  it("reports a transcript error", async () => {
    const { d } = deps([]);
    vi.mocked(d.getTranscript).mockResolvedValue({
      status: "error",
      error: { code: "DB_ERROR", message: "boom", retryable: false },
    });
    await expect(recap(d, "s1", 5, plan)).rejects.toThrow("boom");
  });
});
