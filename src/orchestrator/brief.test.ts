import { describe, expect, it, vi } from "vitest";
import type { BriefSource, LlmRequest } from "@/ipc";
import { mockAnswerBus } from "@/ipc/testing";
import { BRIEF_SYSTEM, briefPrompt, NO_MATERIAL, writeBrief, type BriefDeps } from "./brief";

const plan = { provider: "anthropic", full: "big", cue: "small" };

function deps(material: BriefSource[], reply = "## Summary\n- It's about pricing\n") {
  const requests: LlmRequest[] = [];
  const d: BriefDeps = {
    events: mockAnswerBus.events,
    llmStream: vi.fn((requestId: string, _p: string | null, req: LlmRequest) => {
      requests.push(req);
      queueMicrotask(() => {
        mockAnswerBus.emitToken({ requestId, delta: reply });
        mockAnswerBus.emitDone({
          requestId,
          usage: { inputTokens: 1, outputTokens: 1 },
          finishReason: "end_turn",
        });
      });
      return Promise.resolve({ status: "ok" as const, data: null });
    }),
    briefMaterial: vi.fn(() => Promise.resolve({ status: "ok" as const, data: material })),
    saveBrief: vi.fn((_s: string, contentMd: string) =>
      Promise.resolve({ status: "ok" as const, data: { contentMd, createdAt: 1 } }),
    ),
  };
  return { d, requests };
}

describe("pre-meeting brief (PRD CTX-4)", () => {
  it("fences each document with its name and pack", () => {
    const p = briefPrompt([
      { documentName: 'spec "v2".md', packName: null, text: " Pro: 600/min \n" },
      { documentName: "prices.xlsx", packName: "Sales kit", text: "Pro: $49" },
    ]);
    expect(p).toBe(
      '<documents>\n<document name="spec \'v2\'.md">\nPro: 600/min\n</document>\n\n<document name="prices.xlsx" pack="Sales kit">\nPro: $49\n</document>\n</documents>',
    );
  });

  it("asks the main model for the three sections and saves the brief", async () => {
    const { d, requests } = deps([{ documentName: "a.md", packName: null, text: "Alpha" }]);
    const brief = await writeBrief(d, "s1", plan);
    expect(brief.contentMd).toBe("## Summary\n- It's about pricing");
    expect(d.saveBrief).toHaveBeenCalledWith("s1", "## Summary\n- It's about pricing");
    expect(requests[0]?.model).toBe("big");
    expect(requests[0]?.system).toBe(BRIEF_SYSTEM);
    expect(BRIEF_SYSTEM).toContain("## Summary");
    expect(BRIEF_SYSTEM).toContain("## Key numbers");
    expect(BRIEF_SYSTEM).toContain("## Likely questions");
    expect(requests[0]?.messages[0]?.content).toContain("Alpha");
  });

  it("without documents it says what to do and calls no model", async () => {
    const { d, requests } = deps([]);
    await expect(writeBrief(d, "s1", plan)).rejects.toThrow(NO_MATERIAL);
    expect(requests).toHaveLength(0);
  });

  it("an empty reply is not saved", async () => {
    const { d } = deps([{ documentName: "a.md", packName: null, text: "x" }], "  ");
    await expect(writeBrief(d, "s1", plan)).rejects.toThrow("empty brief");
    expect(d.saveBrief).not.toHaveBeenCalled();
  });
});
