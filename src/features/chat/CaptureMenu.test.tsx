import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSessionStore } from "@/features/session";
import type { LlmRequest, Session } from "@/ipc";
import { mockCommands, mockSessionBus, resetMocks, savedAnswers } from "@/ipc/testing";
import { useAnswering, useChatStore } from "@/orchestrator";
import { ChatView } from "./ChatView";

function Harness({ session }: { session: Session }) {
  useAnswering(session);
  return <ChatView />;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Screens", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

describe("capture-and-ask (SCR-1, SCR-4)", () => {
  beforeEach(() => {
    resetMocks();
    vi.restoreAllMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useChatStore.setState({
      sessionId: null,
      turns: [],
      focusTick: 0,
      pending: null,
      capturing: false,
      captureError: null,
    });
  });

  it("captures the screen, then the typed question is answered with the image", async () => {
    const user = userEvent.setup();
    const stream = vi.spyOn(mockCommands, "llmStream");
    const captured: string[] = [];
    void mockSessionBus.events.onScreenshotCaptured((e) => captured.push(e.screenshot.id));
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);

    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
    const chip = await screen.findByRole("status", { name: "Screenshot attached" });
    expect(chip).toHaveTextContent("1568×882");
    expect(
      await within(chip).findByRole("img", { name: "Screenshot preview" }),
    ).toBeInTheDocument();
    expect(captured).toHaveLength(1);
    const box = screen.getByRole("textbox", { name: "Ask a question" });
    expect(box).toHaveFocus();
    expect(box).toHaveAttribute("placeholder", "What do you want to know about this screenshot?");

    await user.type(box, "What does this error mean?{Enter}");
    await wait(200);
    expect(screen.queryByRole("status", { name: "Screenshot attached" })).not.toBeInTheDocument();

    const requests = stream.mock.calls.map((c) => c[2] as LlmRequest);
    expect(requests).toHaveLength(2);
    // Both cue and full answer see the screenshot when the question is about it.
    for (const r of requests) expect(r.messages.at(-1)?.screenshotId).toBe(captured[0]);
    const turn = screen.getByRole("article", { name: "Chat: What does this error mean?" });
    expect(within(turn).getByText(/Mock answer \(with screenshot\) to:/)).toBeInTheDocument();
    expect(within(turn).getByRole("list", { name: "Sources" })).toHaveTextContent(
      /Screenshot \d\d:\d\d/,
    );
    expect(savedAnswers[0]?.answer.source).toBe("screenshot");
    expect(savedAnswers[0]?.answer.sources).toEqual([
      expect.objectContaining({ type: "screenshot", ref: `s:${captured[0]}` }),
    ]);
  });

  it("captures a chosen window", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    render(<Harness session={s} />);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "A window…" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose a window" });
    await user.click(await within(dialog).findByRole("button", { name: /Browser.*Dashboard/ }));
    expect(await screen.findByRole("status", { name: "Screenshot attached" })).toHaveTextContent(
      "882×1568",
    );
  });

  it("captures a dragged area at full resolution", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    const capture = vi.spyOn(mockCommands, "captureScreenshot");
    render(<Harness session={s} />);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "An area…" }));
    const surface = await screen.findByTestId("region-surface");
    await screen.findByRole("img", { name: "Your screen" });
    vi.spyOn(surface, "getBoundingClientRect").mockReturnValue({
      left: 0,
      top: 0,
      width: 400,
      height: 225,
      right: 400,
      bottom: 225,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    const capture_btn = screen.getByRole("button", { name: "Capture area" });
    expect(capture_btn).toBeDisabled();
    fireEvent.pointerDown(surface, { clientX: 100, clientY: 45, pointerId: 1 });
    fireEvent.pointerMove(surface, { clientX: 300, clientY: 180, pointerId: 1 });
    fireEvent.pointerUp(surface, { clientX: 300, clientY: 180, pointerId: 1 });
    await user.click(capture_btn);
    expect(capture).toHaveBeenCalledWith(s.id, {
      mode: "region",
      region: { x: 0.25, y: 0.2, width: 0.5, height: expect.closeTo(0.6, 6) as number },
    });
    expect(await screen.findByRole("status", { name: "Screenshot attached" })).toHaveTextContent(
      "1280×864",
    );
  });

  it("removing the screenshot asks a plain chat question", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
    await user.click(await screen.findByRole("button", { name: "Remove screenshot" }));
    await user.type(screen.getByRole("textbox", { name: "Ask a question" }), "Plain?{Enter}");
    await wait(200);
    expect(savedAnswers[0]?.answer.source).toBe("chat");
  });

  it("shows capture errors", async () => {
    const user = userEvent.setup();
    vi.spyOn(mockCommands, "captureScreenshot").mockResolvedValue({
      status: "error",
      error: {
        code: "PERMISSION_DENIED",
        message: "Allow Sidekick under Screen Recording.",
        retryable: false,
      },
    });
    const s = await liveSession();
    render(<Harness session={s} />);
    await user.click(screen.getByRole("button", { name: "Screenshot" }));
    await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Screen Recording");
  });
});
