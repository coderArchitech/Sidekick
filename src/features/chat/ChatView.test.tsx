import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useSessionStore } from "@/features/session";
import type { LlmRequest, Session } from "@/ipc";
import { mockCommands, mockTranscriptLine, resetMocks, savedAnswers } from "@/ipc/testing";
import { useAnswering, useChatStore } from "@/orchestrator";
import { ChatView } from "./ChatView";

function Harness({ session }: { session: Session }) {
  useAnswering(session);
  return <ChatView />;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Chat");
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  // Something to answer from, so the router calls the model.
  mockTranscriptLine(s.data.id, {
    speakerLabel: "speaker_1",
    isUser: false,
    text: "The Pro tier allows 600 requests per minute.",
    startMs: 0,
    endMs: 2000,
  });
  return started.data.session;
}

describe("Chat tab", () => {
  beforeEach(() => {
    resetMocks();
    vi.restoreAllMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useChatStore.setState({ sessionId: null, turns: [], focusTick: 0 });
  });

  it("is disabled without a session", () => {
    render(<ChatView />);
    expect(screen.getByText("Start a session to chat with its context.")).toBeInTheDocument();
    expect(screen.getByRole("textbox", { name: "Ask a question" })).toBeDisabled();
  });

  it("sends a message, streams an answer and keeps history", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);

    const box = screen.getByRole("textbox", { name: "Ask a question" });
    await user.type(box, "What's the rate limit?{Enter}");
    expect(box).toHaveValue("");
    await wait(200);
    await user.type(box, "And for Free?");
    await user.click(screen.getByRole("button", { name: "Send" }));
    await wait(200);

    const log = screen.getByRole("log", { name: "Chat" });
    const turns = within(log).getAllByRole("article");
    expect(turns.map((t) => t.getAttribute("aria-label"))).toEqual([
      "Chat: What's the rate limit?",
      "Chat: And for Free?",
    ]);
    expect(within(turns[0] as HTMLElement).getByText(/Mock answer to:/)).toBeInTheDocument();
    expect(useChatStore.getState().turns.every((t) => t.status === "done")).toBe(true);
  });

  it("Shift+Enter adds a new line instead of sending", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    render(<Harness session={s} />);
    const box = screen.getByRole("textbox", { name: "Ask a question" });
    await user.type(box, "line one{Shift>}{Enter}{/Shift}line two");
    expect(box).toHaveValue("line one\nline two");
    expect(useChatStore.getState().turns).toHaveLength(0);
  });

  it("a follow-up includes previous turns in the request", async () => {
    const user = userEvent.setup();
    const stream = vi.spyOn(mockCommands, "llmStream");
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    const box = screen.getByRole("textbox", { name: "Ask a question" });
    await user.type(box, "What's the rate limit?{Enter}");
    await wait(200);
    stream.mockClear();
    await user.type(box, "And the burst limit?{Enter}");
    await wait(200);

    const full = stream.mock.calls.map((c) => c[2] as LlmRequest).find((r) => r.maxTokens > 100);
    expect(full).toBeDefined();
    const msgs = full?.messages ?? [];
    expect(msgs[0]).toEqual({ role: "user", content: "What's the rate limit?" });
    expect(msgs[1]?.role).toBe("assistant");
    expect(msgs[1]?.content).toMatch(/Mock answer to:/);
    expect(msgs[2]?.role).toBe("user");
    expect(msgs[2]?.content).toMatch(/Question: And the burst limit\?$/);
  });

  it("stores chat questions with source = chat and reloads them per session", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    const { unmount } = render(<Harness session={s} />);
    await wait(20);
    await user.type(
      screen.getByRole("textbox", { name: "Ask a question" }),
      "Who owns billing?{Enter}",
    );
    await wait(200);
    expect(savedAnswers).toHaveLength(1);
    expect(savedAnswers[0]?.answer).toMatchObject({
      source: "chat",
      directed: true,
      question: "Who owns billing?",
      sessionId: s.id,
    });
    unmount();

    // A fresh overlay loads the session's chat history.
    useChatStore.setState({ sessionId: null, turns: [], focusTick: 0 });
    render(<Harness session={s} />);
    await wait(20);
    const log = screen.getByRole("log", { name: "Chat" });
    expect(
      within(log).getByRole("article", { name: "Chat: Who owns billing?" }),
    ).toBeInTheDocument();
  });

  it("focuses the input when asked to (Ask hotkey)", async () => {
    const s = await liveSession();
    render(<Harness session={s} />);
    const box = screen.getByRole("textbox", { name: "Ask a question" });
    expect(box).not.toHaveFocus();
    act(() => useChatStore.getState().requestFocus());
    expect(box).toHaveFocus();
  });
});

describe("Follow-up chips (PRD QA-9)", () => {
  beforeEach(() => {
    resetMocks();
    vi.restoreAllMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useChatStore.setState({ sessionId: null, turns: [], focusTick: 0, showTick: 0 });
  });

  it("each chip asks its follow-up with the original exchange as history", async () => {
    const user = userEvent.setup();
    const requests: LlmRequest[] = [];
    const real = mockCommands.llmStream.bind(mockCommands);
    vi.spyOn(mockCommands, "llmStream").mockImplementation((id, p, req) => {
      requests.push(req);
      return real(id, p, req);
    });
    const s = await liveSession();
    render(<Harness session={s} />);
    await wait(20);
    await user.type(
      screen.getByRole("textbox", { name: "Ask a question" }),
      "What's the rate limit?{Enter}",
    );
    await wait(200);

    const first = screen.getByRole("article", { name: "Chat: What's the rate limit?" });
    const chips = within(first).getByRole("group", { name: "Follow-ups" });
    expect(
      within(chips)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Shorter", "More detail", "Give an example", "Explain simply"]);
    const before = useChatStore.getState().showTick;
    requests.length = 0;
    await user.click(within(chips).getByRole("button", { name: "Explain simply" }));
    await wait(200);

    expect(
      screen.getByRole("article", { name: "Chat: Explain simply: What's the rate limit?" }),
    ).toBeInTheDocument();
    expect(useChatStore.getState().showTick).toBe(before + 1);
    const full = requests.find((r) => r.maxTokens > 100);
    expect(full?.messages[0]).toEqual({ role: "user", content: "What's the rate limit?" });
    expect(full?.messages[1]?.role).toBe("assistant");
    expect(full?.messages.at(-1)?.content).toContain("no jargon");
    // Saved with its label, so it reloads the same way.
    const saved = await mockCommands.listAnswers(s.id, null);
    expect(saved.status === "ok" && saved.data.map((a) => a.question)).toContain(
      "Explain simply: What's the rate limit?",
    );
  });

  it("no chips while an answer streams", async () => {
    const s = await liveSession();
    vi.spyOn(mockCommands, "llmStream").mockResolvedValue({ status: "ok", data: null });
    const user = userEvent.setup();
    render(<Harness session={s} />);
    await wait(20);
    await user.type(screen.getByRole("textbox", { name: "Ask a question" }), "Hello?{Enter}");
    await wait(50);
    expect(screen.queryByRole("group", { name: "Follow-ups" })).toBeNull();
  });
});
