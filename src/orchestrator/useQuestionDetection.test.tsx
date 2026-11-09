import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { Session } from "@/ipc";
import { mockCommands, mockTranscriptLine, resetMocks } from "@/ipc/testing";
import { useOrchestratorStore } from "./store";
import { useQuestionDetection } from "./useQuestionDetection";

function Harness({ session }: { session: Session | null }) {
  useQuestionDetection(session);
  return null;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function live(autoAnswer: boolean): Promise<Session> {
  await mockCommands.updateSettings({ profile: { name: "Nitesh", nicknames: ["Nitu"] } });
  const s = await mockCommands.createSession("Detect", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  await mockCommands.startSession(s.data.id);
  const updated = await mockCommands.setAutoAnswer(s.data.id, autoAnswer);
  if (updated.status !== "ok") throw new Error("auto");
  return updated.data;
}

const line = (sessionId: string, text: string, isUser = false) =>
  mockTranscriptLine(sessionId, {
    speakerLabel: isUser ? "me" : "speaker_1",
    isUser,
    text,
    startMs: 0,
    endMs: 1000,
  });

describe("useQuestionDetection", () => {
  beforeEach(() => {
    resetMocks();
    useOrchestratorStore.setState({ questions: [] });
  });

  it("with Auto-answer off, only directed questions are marked to answer", async () => {
    const s = await live(false);
    render(<Harness session={s} />);
    await wait(20);
    line(s.id, "What's the uptime SLA?");
    await wait(700);
    line(s.id, "Nitish, can you confirm the dates?");
    await wait(700);
    const qs = useOrchestratorStore.getState().questions;
    expect(qs.map((q) => [q.text, q.directed, q.answer])).toEqual([
      ["Nitish, can you confirm the dates?", true, true],
      ["What's the uptime SLA?", false, false],
    ]);
  });

  it("with Auto-answer on, every question is marked to answer", async () => {
    const s = await live(true);
    render(<Harness session={s} />);
    await wait(20);
    line(s.id, "How do refunds work?");
    await wait(700);
    expect(useOrchestratorStore.getState().questions[0]?.answer).toBe(true);
  });

  it("ignores statements, the user's own questions and draft sessions", async () => {
    const s = await live(true);
    const { rerender } = render(<Harness session={s} />);
    await wait(20);
    line(s.id, "The rate limit is 600 per minute.");
    line(s.id, "What should I tell them?", true);
    await wait(700);
    expect(useOrchestratorStore.getState().questions).toEqual([]);
    rerender(<Harness session={{ ...s, status: "draft" }} />);
    line(s.id, "Any blockers?");
    await wait(700);
    expect(useOrchestratorStore.getState().questions).toEqual([]);
  });
});
