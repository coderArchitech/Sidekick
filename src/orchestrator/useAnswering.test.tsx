import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import type { Session } from "@/ipc";
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
    const s = await mockCommands.createSession("E2E");
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
});
