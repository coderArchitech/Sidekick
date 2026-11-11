import { beforeEach, describe, expect, it, vi } from "vitest";
import type {
  ContextOptions,
  ContextPack,
  LlmRequest,
  NewAnswer,
  Result,
  SidekickError,
  Stage,
} from "@/ipc";
import { mockAnswerBus, resetMocks } from "@/ipc/testing";
import { AnswerRouter, type AnswerDeps, type AnswerView, type AskInput } from "./answerer";
import { GENERAL_LABEL } from "./prompt";

const PACK: ContextPack = {
  transcript: [
    {
      id: 1,
      speakerLabel: "speaker_1",
      isUser: false,
      text: "What's the rate limit?",
      startMs: 0,
      endMs: 900,
    },
  ],
  notes: null,
  chunks: [
    {
      chunkId: 4,
      documentId: "d1",
      documentName: "Spec",
      page: 14,
      text: "Pro: 600 requests/min.",
    },
  ],
  screenshot: null,
};

const ok = <T>(data: T): Result<T, SidekickError> => ({ status: "ok", data });
const tick = () => new Promise((r) => setTimeout(r, 0));

function harness(
  pack: ContextPack = PACK,
  opts: { streamFails?: boolean; webSearch?: AnswerDeps["webSearch"] } = {},
) {
  const streams: { id: string; provider: string; req: LlmRequest }[] = [];
  const cancelled: string[] = [];
  const saved: NewAnswer[] = [];
  const latency: [Stage, number][] = [];
  const queries: string[] = [];
  const optionsSeen: (ContextOptions | null)[] = [];
  let clock = 10_000;
  const deps: AnswerDeps = {
    retrieveContext: (_s, query, options) => {
      queries.push(query);
      optionsSeen.push(options);
      return Promise.resolve(ok(pack));
    },
    llmStream: (id, provider, req) => {
      streams.push({ id, provider, req });
      return Promise.resolve(
        opts.streamFails
          ? {
              status: "error",
              error: {
                code: "KEY_MISSING",
                message: "Add your Anthropic API key in Settings.",
                retryable: false,
              },
            }
          : ok(null),
      );
    },
    cancelStream: (id) => {
      cancelled.push(id);
      return Promise.resolve(true);
    },
    saveAnswer: (a) => {
      saved.push(a);
      return Promise.resolve(ok(`saved-${saved.length}`));
    },
    recordLatency: (stage, ms) => {
      latency.push([stage, ms]);
      return Promise.resolve();
    },
    events: mockAnswerBus.events,
    now: () => clock,
    webSearch: opts.webSearch,
  };
  const views = new Map<string, AnswerView>();
  const router = new AnswerRouter(deps, (v) => views.set(v.id, v));
  return {
    router,
    streams,
    cancelled,
    saved,
    latency,
    queries,
    optionsSeen,
    views,
    advance: (ms: number) => (clock += ms),
  };
}

const input = (text = "What's the rate limit?"): AskInput => ({
  sessionId: "s1",
  text,
  directed: true,
  source: "detected",
  segmentId: 1,
  endedAt: 9_000,
  models: { provider: "anthropic", full: "claude-opus-5-5", cue: "claude-haiku-5-5" },
});

const token = (requestId: string, delta: string) => mockAnswerBus.emitToken({ requestId, delta });
const done = (requestId: string) =>
  mockAnswerBus.emitDone({
    requestId,
    usage: { inputTokens: 300, outputTokens: 40 },
    finishReason: "end_turn",
  });

describe("AnswerRouter", () => {
  beforeEach(resetMocks);

  it("streams a cue and a full answer in parallel on the right models", async () => {
    const h = harness();
    const view = await h.router.ask(input());
    expect(
      h.streams.map((s) => [
        s.id.split(":")[1],
        s.provider,
        s.req.model,
        s.req.effort,
        s.req.maxTokens,
      ]),
    ).toEqual([
      ["cue", "anthropic", "claude-haiku-5-5", "low", 80],
      ["full", "anthropic", "claude-opus-5-5", "medium", 1500],
    ]);
    expect(h.views.get(view.id)?.status).toBe("streaming");
  });

  it("records latency, cites sources and saves the answer when both streams finish", async () => {
    const h = harness();
    const v = await h.router.ask(input());
    h.advance(800);
    token(`${v.id}:cue`, "600 requests per minute");
    h.advance(700);
    token(`${v.id}:full`, "The Pro tier allows 600 requests/min [2].");
    done(`${v.id}:cue`);
    done(`${v.id}:full`);
    await tick();
    const final = h.views.get(v.id);
    expect(final).toMatchObject({
      status: "done",
      cue: "600 requests per minute",
      answerId: "saved-1",
    });
    expect(final?.sources.map((s) => s.label)).toEqual(["Spec, p.14"]);
    expect(h.latency).toEqual([
      ["question_to_cue", 1800],
      ["question_to_answer", 2500],
    ]);
    expect(h.saved[0]).toMatchObject({
      sessionId: "s1",
      directed: true,
      segmentId: 1,
      bodyMd: "The Pro tier allows 600 requests/min [2].",
      sources: [{ type: "doc", ref: "d1#p14", label: "Spec, p.14" }],
      model: "claude-opus-5-5",
      inputTokens: 300,
      outputTokens: 40,
    });
  });

  it("still asks the model when there is no context, so it can answer from general knowledge", async () => {
    const h = harness({ transcript: [], notes: null, chunks: [], screenshot: null });
    const v = await h.router.ask(input("What is Kubernetes?"));
    expect(h.streams.map((s) => s.id.split(":")[1])).toEqual(["cue", "full"]);
    expect(v.status).toBe("streaming");
  });

  it("labels a general-knowledge answer, strips the tag, and saves it with the label", async () => {
    const h = harness();
    const v = await h.router.ask(input("What is Kubernetes?"));
    token(`${v.id}:cue`, "[gen");
    expect(h.views.get(v.id)?.cue).toBe("");
    token(`${v.id}:cue`, "eral] A container orchestrator");
    token(`${v.id}:full`, "[general] Kubernetes runs and scales containers.");
    done(`${v.id}:cue`);
    done(`${v.id}:full`);
    await tick();
    const final = h.views.get(v.id);
    expect(final).toMatchObject({
      status: "done",
      general: true,
      cue: "A container orchestrator",
      body: "Kubernetes runs and scales containers.",
      sources: [],
    });
    expect(h.saved[0]?.bodyMd).toBe(`_${GENERAL_LABEL}_\n\nKubernetes runs and scales containers.`);
  });

  it("an answer from the context is not labelled general", async () => {
    const h = harness();
    const v = await h.router.ask(input());
    token(`${v.id}:full`, "The Pro tier allows 600 requests/min [2].");
    done(`${v.id}:cue`);
    done(`${v.id}:full`);
    await tick();
    expect(h.views.get(v.id)?.general).toBeFalsy();
    expect(h.saved[0]?.bodyMd).toBe("The Pro tier allows 600 requests/min [2].");
  });

  it("a third question cancels the oldest; its tokens are then ignored", async () => {
    const h = harness();
    const a = await h.router.ask(input("A?"));
    await h.router.ask(input("B?"));
    await h.router.ask(input("C?"));
    expect(h.cancelled).toEqual([`${a.id}:cue`, `${a.id}:full`]);
    expect(h.views.get(a.id)?.status).toBe("cancelled");
    token(`${a.id}:full`, "late");
    expect(h.views.get(a.id)?.body).toBe("");
  });

  it("a stream that can't start shows the error on that answer only", async () => {
    const h = harness(PACK, { streamFails: true });
    const v = await h.router.ask(input());
    expect(h.views.get(v.id)).toMatchObject({
      status: "error",
      error: "Add your Anthropic API key in Settings.",
    });
    // The router keeps working for the next question (NFR-5).
    const h2 = harness();
    const next = await h2.router.ask(input("Next?"));
    expect(h2.views.get(next.id)?.status).toBe("streaming");
  });

  it("a failed cue still lets the full answer finish", async () => {
    const h = harness();
    const v = await h.router.ask(input());
    mockAnswerBus.emitError({
      requestId: `${v.id}:cue`,
      error: { code: "RATE_LIMITED", message: "busy", retryable: true },
    });
    token(`${v.id}:full`, "600/min [2]");
    done(`${v.id}:full`);
    await tick();
    expect(h.views.get(v.id)?.status).toBe("done");
  });

  it("a refusal shows a neutral note", async () => {
    const h = harness();
    const v = await h.router.ask(input());
    done(`${v.id}:cue`);
    mockAnswerBus.emitDone({
      requestId: `${v.id}:full`,
      usage: { inputTokens: 1, outputTokens: 0 },
      finishReason: "refusal",
    });
    await tick();
    expect(h.views.get(v.id)?.body).toBe("The model declined to answer this one.");
  });

  it("cancel stops an answer on request", async () => {
    const h = harness();
    const v = await h.router.ask(input());
    h.router.cancel(v.id);
    expect(h.views.get(v.id)?.status).toBe("cancelled");
    expect(h.cancelled).toHaveLength(2);
  });

  it("a full-only Skill skips the cue and still finishes and saves", async () => {
    const h = harness();
    const view = await h.router.ask({
      ...input(),
      skill: { name: "Architect", instructions: "Be precise.", answerFormat: "full_only" },
    });
    expect(h.streams.map((s) => s.id.split(":")[1])).toEqual(["full"]);
    expect(h.streams[0]?.req.system).toContain("Active Skill: Architect\nBe precise.");
    token(`${view.id}:full`, "600 requests/min [2].");
    done(`${view.id}:full`);
    await tick();
    expect(h.views.get(view.id)).toMatchObject({
      status: "done",
      cue: "",
      body: "600 requests/min [2].",
    });
    expect(h.saved).toHaveLength(1);
  });

  it("a follow-up shows and saves its label and searches with the original question (QA-9)", async () => {
    const h = harness();
    const v = await h.router.ask({
      ...input(
        'About "What\'s the rate limit?": Give the same answer again in at most two short sentences.',
      ),
      label: "Shorter: What's the rate limit?",
      query: "What's the rate limit?",
      history: [
        { role: "user", content: "What's the rate limit?" },
        { role: "assistant", content: "600 requests/min." },
      ],
    });
    expect(h.views.get(v.id)?.question).toBe("Shorter: What's the rate limit?");
    expect(h.queries).toEqual(["What's the rate limit?"]);
    const full = h.streams.find((s) => s.id.endsWith(":full"));
    expect(full?.req.messages[0]).toEqual({ role: "user", content: "What's the rate limit?" });
    expect(full?.req.messages.at(-1)?.content).toContain("at most two short sentences");
    token(`${v.id}:cue`, "600/min");
    token(`${v.id}:full`, "600 requests per minute.");
    done(`${v.id}:cue`);
    done(`${v.id}:full`);
    await tick();
    expect(h.saved[0]?.question).toBe("Shorter: What's the rate limit?");
  });

  it("asks about a chosen screenshot through the context options (SCR-5, SCR-6)", async () => {
    const h = harness();
    await h.router.ask(input("Plain question"));
    await h.router.ask({ ...input("About this?"), source: "screenshot", screenshotId: "shot-1" });
    expect(h.optionsSeen).toEqual([null, { windowMs: 180_000, topK: 5, screenshotId: "shot-1" }]);
  });

  it("an agent Skill runs its tools first and the answer sees the results (SKL-4)", async () => {
    const h = harness({ transcript: [], notes: null, chunks: [], screenshot: null });
    const v = h.router.ask({
      ...input("What is 15% of 2400?"),
      skill: { name: "Analyst", instructions: "Be exact.", tools: ["calculator"] },
    });
    // The agent step: the fast model asks for the calculator, then is done.
    await tick();
    const step = h.streams.find((s) => s.id.startsWith("agent:"));
    expect(step?.req.model).toBe("claude-haiku-5-5");
    token(step?.id ?? "", '{"tool": "calculator", "input": "2400 * 15%"}');
    done(step?.id ?? "");
    await tick();
    await tick();
    const step2 = h.streams.filter((s) => s.id.startsWith("agent:"))[1];
    token(step2?.id ?? "", '{"done": true}');
    done(step2?.id ?? "");
    const view = await v;

    // Nothing else to answer from, but the tool result is enough.
    const full = h.streams.find((s) => s.id === `${view.id}:full`);
    const content = full?.req.messages.at(-1)?.content ?? "";
    expect(content).toContain("<tool_results>\n- calculator(2400 * 15%) → 360\n</tool_results>");
    expect(full?.req.system).toContain("Results in <tool_results> come from tools");
    expect(h.views.get(view.id)?.tools).toEqual([
      { tool: "calculator", input: "2400 * 15%", output: "360", ok: true },
    ]);
  });

  it("a Skill without tools makes no agent call", async () => {
    const h = harness();
    await h.router.ask({ ...input(), skill: { name: "Plain", instructions: "Be plain." } });
    expect(h.streams.some((s) => s.id.startsWith("agent:"))).toBe(false);
  });

  it("an agent's web search uses the router's web search (KEY-8)", async () => {
    const webSearch = vi.fn(() =>
      Promise.resolve({
        status: "ok" as const,
        data: [{ title: "Rates", url: "https://x.test", snippet: "4.5%" }],
      }),
    );
    const h = harness(PACK, { webSearch });
    const v = h.router.ask({
      ...input("What's the Fed rate today?"),
      skill: { name: "Analyst", instructions: "Be current.", tools: ["web_search"] },
    });
    await tick();
    const step = h.streams.find((s) => s.id.startsWith("agent:"));
    token(step?.id ?? "", '{"tool": "web_search", "input": "fed rate today"}');
    done(step?.id ?? "");
    await tick();
    await tick();
    const step2 = h.streams.filter((s) => s.id.startsWith("agent:"))[1];
    token(step2?.id ?? "", '{"done": true}');
    done(step2?.id ?? "");
    const view = await v;
    expect(webSearch).toHaveBeenCalledWith("fed rate today");
    const full = h.streams.find((s) => s.id === `${view.id}:full`);
    expect(full?.req.messages.at(-1)?.content).toContain(
      "web_search(fed rate today) → Rates (https://x.test): 4.5%",
    );
  });
});
