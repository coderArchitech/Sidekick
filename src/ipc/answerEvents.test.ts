import { beforeEach, describe, expect, it } from "vitest";
import { mockAnswerBus, mockCommands, resetMocks } from "./mock";

const request = {
  model: "mock-fast",
  system: null,
  messages: [{ role: "user" as const, content: "Rate limit?" }],
  maxTokens: 100,
  effort: null,
};

describe("mock answer streaming", () => {
  beforeEach(resetMocks);

  it("streams tokens then exactly one done event", async () => {
    const tokens: string[] = [];
    const done = new Promise<string>((resolve) => {
      void mockAnswerBus.events.onDone((e) => resolve(e.finishReason));
    });
    await mockAnswerBus.events.onToken((e) => tokens.push(e.delta));
    await mockCommands.llmStream("r1", "anthropic", request);
    expect(await done).toBe("end_turn");
    expect(tokens.join("")).toBe("Mock answer to: Rate limit?");
  });

  it("cancel emits CANCELLED and stops tokens", async () => {
    const errors: string[] = [];
    await mockAnswerBus.events.onError((e) => errors.push(e.error.code));
    await mockCommands.llmStream("r2", "anthropic", request);
    expect(await mockCommands.cancelStream("r2")).toBe(true);
    expect(errors).toEqual(["CANCELLED"]);
    expect(await mockCommands.cancelStream("r2")).toBe(false);
  });

  it("unlisten stops delivery", async () => {
    const seen: string[] = [];
    const unlisten = await mockAnswerBus.events.onToken((e) => seen.push(e.delta));
    unlisten();
    mockAnswerBus.emitToken({ requestId: "x", delta: "hi" });
    expect(seen).toEqual([]);
  });
});

describe("mock key commands", () => {
  beforeEach(resetMocks);

  it("saves keys and exposes only the last 4 characters", async () => {
    const result = await mockCommands.setApiKey("anthropic", "sk-ant-abcd1234");
    expect(result).toEqual({
      status: "ok",
      data: { key: { saved: true, last4: "1234" }, verified: true },
    });
    expect(JSON.stringify(await mockCommands.listProviders("llm"))).not.toContain("abcd");
  });

  it("rejects keys containing 'invalid' and unknown providers", async () => {
    const bad = await mockCommands.setApiKey("anthropic", "invalid-key");
    expect(bad.status === "error" && bad.error.code).toBe("KEY_INVALID");
    const unknown = await mockCommands.setApiKey("nope", "x");
    expect(unknown.status).toBe("error");
  });
});
