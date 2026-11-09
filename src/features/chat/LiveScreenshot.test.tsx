import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { LiveAnswers } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import type { LlmRequest, Session } from "@/ipc";
import { mockCommands, resetMocks, savedAnswers } from "@/ipc/testing";
import { useAnswering, useAnswersStore, useChatStore } from "@/orchestrator";
import { LiveScreenshot } from "./LiveScreenshot";

function Harness({ session }: { session: Session }) {
  useAnswering(session);
  return (
    <>
      <LiveScreenshot />
      <LiveAnswers />
    </>
  );
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Interview", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

describe("screenshots on Live Answers are read as the question", () => {
  beforeEach(() => {
    resetMocks();
    vi.restoreAllMocks();
    useAnswersStore.getState().clear();
    useChatStore.setState({ sessionId: null, turns: [], screenshots: [], pending: null });
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
  });

  it("the Screenshot button answers what's on screen in Live Answers", async () => {
    const user = userEvent.setup();
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    // Opens below the button (it sits at the top of the tab).
    expect(screen.getByRole("menu", { name: "Capture" })).toHaveClass("top-full");
    await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
    await wait(200);

    const card = screen.getByRole("article", { name: "Answer to: Question on screen" });
    expect(within(card).getAllByText(/Mock answer \(with screenshot\) to:/).length).toBeGreaterThan(
      0,
    );
    const shot = useChatStore.getState().screenshots[0];
    expect(shot).toBeTruthy();
    const requests = stream.mock.calls.map((c) => c[2] as LlmRequest);
    expect(requests.length).toBeGreaterThan(0);
    for (const r of requests) expect(r.messages.at(-1)?.screenshotId).toBe(shot?.id);
    expect(requests[0]?.messages.at(-1)?.content).toContain(
      "There is a question on this screenshot",
    );
    // Shown in Live Answers, not Chat; saved as a screenshot question.
    expect(useChatStore.getState().turns).toHaveLength(0);
    expect(savedAnswers[0]?.answer).toMatchObject({
      source: "screenshot",
      question: "Question on screen",
    });
  });

  it("explains a failed capture", async () => {
    const user = userEvent.setup();
    vi.spyOn(mockCommands, "captureScreenshot").mockResolvedValue({
      status: "error",
      error: {
        code: "PERMISSION_DENIED",
        message: "Allow screen recording in System Settings.",
        retryable: false,
      },
    });
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Allow screen recording");
  });
});
