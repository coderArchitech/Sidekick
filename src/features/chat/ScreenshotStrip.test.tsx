import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSessionStore } from "@/features/session";
import type { LlmRequest, Session } from "@/ipc";
import { mockCommands, resetMocks, savedAnswers } from "@/ipc/testing";
import { SCREENSHOT_ACTIONS, useAnswering, useChatStore } from "@/orchestrator";
import { ChatView } from "./ChatView";

function Harness({ session }: { session: Session }) {
  useAnswering(session);
  return <ChatView />;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Screens");
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

async function capture(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Screenshot" }));
  await user.click(screen.getByRole("menuitem", { name: "Full screen" }));
  await screen.findByRole("status", { name: "Screenshot attached" });
}

describe("screenshot quick actions and strip (PRD SCR-5, SCR-6)", () => {
  beforeEach(() => {
    resetMocks();
    vi.restoreAllMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useChatStore.setState({
      sessionId: null,
      turns: [],
      focusTick: 0,
      showTick: 0,
      pending: null,
      capturing: false,
      captureError: null,
      screenshots: [],
    });
  });

  it("each quick action sends its own prompt with that screenshot", async () => {
    const user = userEvent.setup();
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);

    const seen = new Set<string>();
    for (const action of SCREENSHOT_ACTIONS) {
      await capture(user);
      const shot = useChatStore.getState().pending;
      stream.mockClear();
      const group = screen.getByRole("group", { name: "Quick actions" });
      await user.click(within(group).getByRole("button", { name: action.label }));
      await wait(150);
      expect(screen.queryByRole("status", { name: "Screenshot attached" })).toBeNull();
      const requests = stream.mock.calls.map((c) => c[2] as LlmRequest);
      const full = requests.find((r) => r.maxTokens > 100);
      expect(full?.messages.at(-1)?.content).toContain(action.prompt);
      expect(full?.messages.at(-1)?.screenshotId).toBe(shot?.id);
      expect(
        screen.getByRole("article", { name: `Chat: ${action.label} (screenshot)` }),
      ).toBeInTheDocument();
      seen.add(action.prompt);
    }
    expect(seen.size).toBe(5);
    expect(savedAnswers.every((a) => a.answer.source === "screenshot")).toBe(true);
  });

  it("the strip lists every capture; re-ask uses that exact screenshot", async () => {
    const user = userEvent.setup();
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await capture(user);
    const first = useChatStore.getState().pending;
    await capture(user);
    await user.click(screen.getByRole("button", { name: "Remove screenshot" }));

    const strip = screen.getByRole("region", { name: "Screenshots" });
    expect(strip).toHaveTextContent("Screenshots (2)");
    const asks = within(strip).getAllByRole("button", { name: /^Ask about the screenshot/ });
    expect(asks).toHaveLength(2);

    // Re-ask the older one.
    await user.click(asks[0] as HTMLElement);
    expect(asks[0]).toHaveAttribute("aria-pressed", "true");
    expect(useChatStore.getState().pending?.id).toBe(first?.id);
    stream.mockClear();
    await user.type(
      screen.getByRole("textbox", { name: "Ask a question" }),
      "And this one?{Enter}",
    );
    await wait(150);
    const full = stream.mock.calls.map((c) => c[2] as LlmRequest).find((r) => r.maxTokens > 100);
    expect(full?.messages.at(-1)?.screenshotId).toBe(first?.id);
  });

  it("deleting a capture removes it (and detaches it)", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await capture(user);
    const strip = screen.getByRole("region", { name: "Screenshots" });
    await user.click(within(strip).getByRole("button", { name: /^Delete the screenshot/ }));
    await wait(20);
    expect(screen.queryByRole("region", { name: "Screenshots" })).toBeNull();
    expect(screen.queryByRole("status", { name: "Screenshot attached" })).toBeNull();
    const left = await mockCommands.listScreenshots(s.id);
    expect(left.status === "ok" && left.data).toEqual([]);
  });

  it("the strip reloads with the session", async () => {
    const s = await liveSession();
    await mockCommands.captureScreenshot(s.id, { mode: "screen" });
    await mockCommands.captureScreenshot(s.id, { mode: "screen" });
    render(<Harness session={s} />);
    expect(await screen.findByText("Screenshots (2)")).toBeInTheDocument();
  });

  it("has no accessibility violations (axe)", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    const { container } = render(<Harness session={s} />);
    await wait(20);
    await capture(user);
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});
