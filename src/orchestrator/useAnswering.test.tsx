import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LlmRequest, Session } from "@/ipc";
import { mockCommands, mockTranscriptLine, resetMocks, savedAnswers } from "@/ipc/testing";
import { useAnswersStore } from "./answersStore";
import { useOrchestratorStore } from "./store";
import { useAnswering } from "./useAnswering";
import { useQuestionDetection } from "./useQuestionDetection";

function Harness({ session }: { session: Session }) {
  useQuestionDetection(session);
  useAnswering(session);
  return null;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

describe("detect → answer → save", () => {
  beforeEach(() => {
    resetMocks();
    useOrchestratorStore.setState({ questions: [] });
    useAnswersStore.setState({ views: [] });
  });

  it("answers a detected question end to end with the mocks", async () => {
    const s = await mockCommands.createSession("E2E", null);
    if (s.status !== "ok") throw new Error("create");
    await mockCommands.confirmConsent(s.data.id, "participants_agreed");
    const started = await mockCommands.startSession(s.data.id);
    if (started.status !== "ok") throw new Error("start");
    render(<Harness session={started.data.session} />);
    await wait(20);
    mockTranscriptLine(s.data.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "What's the rate limit for the Pro tier?",
      startMs: 0,
      endMs: 1200,
    });
    await wait(900);
    const [view] = useAnswersStore.getState().views;
    expect(view).toMatchObject({
      question: "What's the rate limit for the Pro tier?",
      status: "done",
      source: "detected",
    });
    expect(view?.cue.startsWith("Mock answer to:")).toBe(true);
    expect(savedAnswers).toHaveLength(1);
    expect(view?.answerId).toBe(savedAnswers[0]?.id);
  });

  it("uses the session's active Skill in the prompt", async () => {
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await mockCommands.createSession("Skilled", null);
    if (s.status !== "ok") throw new Error("create");
    await mockCommands.confirmConsent(s.data.id, "participants_agreed");
    await mockCommands.startSession(s.data.id);
    const withSkill = await mockCommands.setSessionSkill(s.data.id, "teacher");
    if (withSkill.status !== "ok") throw new Error("skill");
    render(<Harness session={withSkill.data} />);
    await wait(20);
    mockTranscriptLine(s.data.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "Why is the sky blue?",
      startMs: 0,
      endMs: 1200,
    });
    await wait(900);
    const all = stream.mock.calls.map((c) => (c[2] as LlmRequest).system ?? "");
    // Teacher lists tools, so an agent step runs first (it needs none here).
    const agent = all.filter((sys) => sys.startsWith("You decide whether tools are needed"));
    expect(agent).toHaveLength(1);
    const systems = all.filter((sys) => !agent.includes(sys));
    expect(systems).toHaveLength(2);
    for (const sys of systems) expect(sys).toContain("Active Skill: Teacher");
    stream.mockRestore();
  });

  it("answers with the session's own provider (KEY-9)", async () => {
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await mockCommands.createSession("Override", {
      sttProvider: null,
      llmProvider: "openai",
    });
    if (s.status !== "ok") throw new Error("create");
    await mockCommands.confirmConsent(s.data.id, "participants_agreed");
    const started = await mockCommands.startSession(s.data.id);
    if (started.status !== "ok") throw new Error("start");
    render(<Harness session={started.data.session} />);
    await wait(20);
    mockTranscriptLine(s.data.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "What's the rate limit for the Pro tier?",
      startMs: 0,
      endMs: 1200,
    });
    await wait(900);
    const calls = stream.mock.calls.map((c) => [c[1], (c[2] as LlmRequest).model]);
    expect(calls).toEqual([
      ["openai", "gpt-5-mini"],
      ["openai", "gpt-5"],
    ]);
    stream.mockRestore();
  });
});
