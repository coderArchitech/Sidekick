import { describe, expect, it, vi } from "vitest";
import type { ContextPack, LlmRequest } from "@/ipc";
import { mockAnswerBus } from "@/ipc/testing";
import { agentSystem, calculate, formatNumber, parseStep, runAgent, type AgentDeps } from "./agent";

const plan = { provider: "anthropic", full: "big", cue: "small" };

describe("calculator (no eval)", () => {
  it("does arithmetic with precedence, powers, percent and parentheses", () => {
    expect(calculate("2 + 3 * 4")).toBe(14);
    expect(calculate("(2 + 3) * 4")).toBe(20);
    expect(calculate("2 ^ 3 ^ 2")).toBe(512);
    expect(calculate("2400 * 15%")).toBe(360);
    expect(calculate("-5 + 2")).toBe(-3);
    expect(calculate("1,200 / 4")).toBe(300);
    expect(calculate("6 × 7 ÷ 2")).toBe(21);
    expect(formatNumber(calculate("0.1 + 0.2"))).toBe("0.3");
  });

  it("refuses anything that isn't arithmetic", () => {
    for (const bad of ["alert(1)", "2 +", "1 / 0", "(1 + 2", "", "2 ** x", "9".repeat(201)])
      expect(() => calculate(bad), bad).toThrow();
  });
});

describe("agent step parsing", () => {
  it("reads a tool call or done, and nothing else", () => {
    expect(parseStep('Sure: {"tool": "calculator", "input": " 2+2 "}')).toEqual({
      tool: "calculator",
      input: "2+2",
    });
    expect(parseStep('{"done": true}')).toEqual({ done: true });
    expect(parseStep('{"tool": "calculator", "input": ""}')).toBeNull();
    expect(parseStep("Mock answer to: …")).toBeNull();
    expect(parseStep("{oops}")).toBeNull();
  });

  it("the prompt lists only the Skill's tools", () => {
    const s = agentSystem(["calculator"]);
    expect(s).toContain("calculator:");
    expect(s).not.toContain("doc_search:");
    expect(s).not.toContain("web_search:");
  });
});

const PACK: ContextPack = {
  transcript: [],
  notes: null,
  chunks: [
    { chunkId: 1, documentId: "d", documentName: "Spec", page: 4, text: "Pro: 600 requests/min." },
  ],
  screenshot: null,
};

/** A fake model that replies with the given steps in order. */
function deps(steps: string[], extra: Partial<AgentDeps> = {}) {
  const requests: LlmRequest[] = [];
  let i = 0;
  const d: AgentDeps = {
    events: mockAnswerBus.events,
    llmStream: vi.fn((requestId: string, _p: string | null, req: LlmRequest) => {
      requests.push(req);
      const text = steps[i++] ?? '{"done": true}';
      queueMicrotask(() => {
        mockAnswerBus.emitToken({ requestId, delta: text });
        mockAnswerBus.emitDone({
          requestId,
          usage: { inputTokens: 1, outputTokens: 1 },
          finishReason: "end_turn",
        });
      });
      return Promise.resolve({ status: "ok" as const, data: null });
    }),
    retrieveContext: vi.fn(() => Promise.resolve({ status: "ok" as const, data: PACK })),
    ...extra,
  };
  return { d, requests };
}

const ask = (
  d: AgentDeps,
  tools: ("calculator" | "doc_search" | "web_search")[],
  limits?: { maxCalls: number; timeoutMs: number },
) => runAgent(d, { sessionId: "s1", question: "What is 15% of 2400?", tools, plan }, limits);

describe("agent tool loop (PRD SKL-4)", () => {
  it("runs allowed tools and feeds results to the next step", async () => {
    const { d, requests } = deps([
      '{"tool": "calculator", "input": "2400 * 15%"}',
      '{"tool": "doc_search", "input": "Pro limit"}',
      '{"done": true}',
    ]);
    const results = await ask(d, ["calculator", "doc_search"]);
    expect(results).toEqual([
      { tool: "calculator", input: "2400 * 15%", output: "360", ok: true },
      {
        tool: "doc_search",
        input: "Pro limit",
        output: "Spec p.4: Pro: 600 requests/min.",
        ok: true,
      },
    ]);
    expect(d.retrieveContext).toHaveBeenCalledWith("s1", "Pro limit", {
      windowMs: 0,
      topK: 3,
      screenshotId: null,
    });
    expect(requests[0]?.model).toBe("small");
    expect(requests[1]?.messages[0]?.content).toContain("calculator(2400 * 15%) → 360");
  });

  it("never runs a tool outside the Skill's allowlist", async () => {
    const { d } = deps(['{"tool": "web_search", "input": "secret"}']);
    const results = await ask(d, ["calculator"]);
    expect(results).toEqual([
      {
        tool: "web_search",
        input: "secret",
        output: "That tool isn't allowed for this Skill.",
        ok: false,
      },
    ]);
    const { d: d2 } = deps(['{"tool": "shell", "input": "rm -rf /"}']);
    expect((await ask(d2, ["calculator"]))[0]?.ok).toBe(false);
  });

  it("stops at the call limit, on a repeat, and at the time limit", async () => {
    const call = (n: number) => `{"tool": "calculator", "input": "${n} + 1"}`;
    const { d } = deps([call(1), call(2), call(3), call(4)]);
    expect(await ask(d, ["calculator"], { maxCalls: 2, timeoutMs: 10_000 })).toHaveLength(2);

    const { d: d2 } = deps([call(1), call(1), call(2)]);
    expect(await ask(d2, ["calculator"])).toHaveLength(1);

    let t = 0;
    const { d: d3 } = deps([call(1), call(2), call(3)], { now: () => (t += 5_000) });
    expect((await ask(d3, ["calculator"], { maxCalls: 3, timeoutMs: 8_000 })).length).toBeLessThan(
      3,
    );
  });

  it("tool and model failures never throw", async () => {
    const { d } = deps([
      '{"tool": "calculator", "input": "1 / 0"}',
      '{"tool": "web_search", "input": "x"}',
    ]);
    const results = await ask(d, ["calculator", "web_search"]);
    expect(results.map((r) => [r.tool, r.ok, r.output])).toEqual([
      ["calculator", false, "Division by zero"],
      ["web_search", false, "Web search isn't set up (Settings → Web search)."],
    ]);

    const failing: AgentDeps = {
      ...deps([]).d,
      llmStream: () =>
        Promise.resolve({
          status: "error",
          error: { code: "KEY_MISSING", message: "no key", retryable: false },
        }),
    };
    expect(await ask(failing, ["calculator"])).toEqual([]);
  });

  it("uses web search when it's set up", async () => {
    const webSearch = vi.fn(() =>
      Promise.resolve({
        status: "ok" as const,
        data: [{ title: "Rates", url: "https://x.test", snippet: "Fed rate 4.5%" }],
      }),
    );
    const { d } = deps(['{"tool": "web_search", "input": "fed rate"}'], { webSearch });
    const [r] = await ask(d, ["web_search"]);
    expect(r).toEqual({
      tool: "web_search",
      input: "fed rate",
      output: "Rates (https://x.test): Fed rate 4.5%",
      ok: true,
    });
  });

  it("does nothing without tools", async () => {
    const { d, requests } = deps(['{"tool": "calculator", "input": "1+1"}']);
    expect(await ask(d, [])).toEqual([]);
    expect(requests).toHaveLength(0);
  });
});
