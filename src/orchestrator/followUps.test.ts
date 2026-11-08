import { describe, expect, it } from "vitest";
import type { AnswerView } from "./answerer";
import { canFollowUp, FOLLOW_UPS, followUpInput } from "./followUps";

const view = (over: Partial<AnswerView> = {}): AnswerView => ({
  id: "a1",
  question: "What's the rate limit?",
  directed: false,
  source: "detected",
  cue: "600/min",
  body: "The Pro tier allows 600 requests/min [1].",
  sources: [],
  status: "done",
  error: null,
  answerId: "x",
  ...over,
});

describe("follow-up chips (PRD QA-9)", () => {
  it("offers the four PRD chips in order", () => {
    expect(FOLLOW_UPS.map((f) => f.label)).toEqual([
      "Shorter",
      "More detail",
      "Give an example",
      "Explain simply",
    ]);
  });

  it("each chip sends its own instruction with the original exchange", () => {
    const seen = new Set<string>();
    for (const f of FOLLOW_UPS) {
      const i = followUpInput(view(), f.kind);
      expect(i.label).toBe(`${f.label}: What's the rate limit?`);
      expect(i.text).toBe(`About "What's the rate limit?": ${f.instruction}`);
      expect(i.query).toBe("What's the rate limit?");
      expect(i.history).toEqual([
        { role: "user", content: "What's the rate limit?" },
        { role: "assistant", content: "The Pro tier allows 600 requests/min [1]." },
      ]);
      seen.add(i.text);
    }
    expect(seen.size).toBe(4);
    expect(followUpInput(view(), "shorter").text).toContain("two short sentences");
    expect(followUpInput(view(), "example").text).toContain("example");
    expect(followUpInput(view(), "simple").text).toContain("no jargon");
    expect(followUpInput(view(), "detail").text).toContain("more detail");
  });

  it("a chip on a follow-up refers back to the first question", () => {
    const i = followUpInput(view({ question: "Shorter: What's the rate limit?" }), "example");
    expect(i.label).toBe("Give an example: What's the rate limit?");
    expect(i.query).toBe("What's the rate limit?");
  });

  it("falls back to the cue when there is no body", () => {
    expect(followUpInput(view({ body: "" }), "detail").history[1]?.content).toBe("600/min");
  });

  it("only finished answers with text take follow-ups", () => {
    expect(canFollowUp(view())).toBe(true);
    expect(canFollowUp(view({ status: "streaming" }))).toBe(false);
    expect(canFollowUp(view({ status: "error" }))).toBe(false);
    expect(canFollowUp(view({ body: "", cue: " " }))).toBe(false);
  });
});
