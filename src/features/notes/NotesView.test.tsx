import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import axe from "axe-core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSessionStore } from "@/features/session";
import type { LlmRequest, Session } from "@/ipc";
import { mockAnswerBus, mockCommands, mockTranscriptLine, resetMocks } from "@/ipc/testing";
import { NOTES_INTERVAL_MS, useNotes, useNotesStore } from "@/orchestrator";
import { NotesView } from "./NotesView";

function Harness() {
  const session = useSessionStore((s) => s.session);
  useNotes(session);
  return <NotesView />;
}

const RECAP =
  '{"summary": "The team chose the Pro tier.", "decisions": ["Ship on Friday"], "actionItems": [{"text": "Send the quote", "owner": "Priya", "due": "Monday"}]}';

/** Notes calls get notes text, the recap gets JSON; everything else uses the default mock. */
function fakeModel() {
  const calls: LlmRequest[] = [];
  vi.spyOn(mockCommands, "llmStream").mockImplementation((requestId, _p, req) => {
    calls.push(req);
    const text = req.system?.includes("recap")
      ? RECAP
      : `## Topics\n- Notes v${calls.length}\n## Decisions\n- None yet\n## Open questions\n- None yet`;
    queueMicrotask(() => {
      mockAnswerBus.emitToken({ requestId, delta: text });
      mockAnswerBus.emitDone({
        requestId,
        usage: { inputTokens: 1, outputTokens: 1 },
        finishReason: "end_turn",
      });
    });
    return Promise.resolve({ status: "ok", data: null });
  });
  return calls;
}

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Notes");
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

const say = (sessionId: string, text: string, endMs: number) =>
  mockTranscriptLine(sessionId, {
    speakerLabel: "speaker_1",
    isUser: false,
    text,
    startMs: endMs - 1000,
    endMs,
  });

const advance = (ms: number) => act(() => vi.advanceTimersByTimeAsync(ms));

async function setStatus(status: "paused" | "live" | "ended") {
  const id = useSessionStore.getState().session?.id ?? "";
  const r =
    status === "paused"
      ? await mockCommands.pauseSession(id)
      : status === "live"
        ? await mockCommands.resumeSession(id)
        : await mockCommands.endSession(id);
  if (r.status !== "ok") throw new Error(status);
  act(() => useSessionStore.setState({ session: r.data }));
}

describe("Notes tab and rolling notes job", () => {
  beforeEach(() => {
    vi.useFakeTimers({ shouldAdvanceTime: false });
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useNotesStore.setState({ sessionId: null, notes: null, activity: "idle", error: null });
  });
  afterEach(() => {
    vi.restoreAllMocks();
    vi.useRealTimers();
  });

  it("asks for a session first", () => {
    render(<NotesView />);
    expect(screen.getByText("Start a session to keep notes.")).toBeInTheDocument();
  });

  it("runs every 45 s, only while live and only with new transcript", async () => {
    const calls = fakeModel();
    const s = await liveSession();
    render(<Harness />);
    await advance(10);
    expect(screen.getByText(/Notes appear here as the meeting goes on/)).toBeInTheDocument();

    await advance(NOTES_INTERVAL_MS);
    expect(calls).toHaveLength(0); // nothing said yet

    say(s.id, "Let's discuss pricing.", 2000);
    await advance(NOTES_INTERVAL_MS - 100);
    expect(calls).toHaveLength(0); // not yet 45 s
    await advance(200);
    expect(calls).toHaveLength(1);

    // Sections render and update live.
    const notes = screen.getByRole("region", { name: "Notes" });
    expect(within(notes).getByText("Topics")).toBeInTheDocument();
    expect(within(notes).getByText("Decisions")).toBeInTheDocument();
    expect(within(notes).getByText("Open questions")).toBeInTheDocument();
    expect(within(notes).getByText("Notes v1")).toBeInTheDocument();

    await advance(NOTES_INTERVAL_MS);
    expect(calls).toHaveLength(1); // no new transcript

    await setStatus("paused");
    say(s.id, "Said while paused.", 4000);
    await advance(NOTES_INTERVAL_MS * 2);
    expect(calls).toHaveLength(1); // paused: no job

    await setStatus("live");
    await advance(NOTES_INTERVAL_MS);
    expect(calls).toHaveLength(2);
    expect(within(notes).getByText("Notes v2")).toBeInTheDocument();
    expect(useNotesStore.getState().notes?.version).toBe(2);
  });

  it("ending the session writes a summary with decisions and action items", async () => {
    const calls = fakeModel();
    const s = await liveSession();
    render(<Harness />);
    await advance(10);
    say(s.id, "Priya will send the quote by Monday. We ship Friday.", 3000);
    await setStatus("ended");
    await advance(50);

    const recap = calls.find((c) => c.system?.includes("recap"));
    expect(recap?.messages[0]?.content).toContain("Priya will send the quote");
    const summary = screen.getByRole("region", { name: "Summary" });
    expect(within(summary).getByText("The team chose the Pro tier.")).toBeInTheDocument();
    expect(within(summary).getByText("Ship on Friday")).toBeInTheDocument();
    const items = within(summary).getByRole("list", { name: "Action items" });
    expect(items).toHaveTextContent("Send the quote — Priya (due Monday)");

    // Saved, so a fresh overlay shows it without another model call.
    const saved = await mockCommands.getNotes(s.id);
    expect(saved.status === "ok" && saved.data.actionItems[0]?.owner).toBe("Priya");
  });

  it("a failed update shows a notice and keeps the old notes", async () => {
    fakeModel();
    let failing = false;
    const realSave = mockCommands.saveNotes.bind(mockCommands);
    vi.spyOn(mockCommands, "saveNotes").mockImplementation((...args) =>
      failing
        ? Promise.resolve({
            status: "error",
            error: { code: "DB_ERROR", message: "Disk is full.", retryable: true },
          })
        : realSave(...args),
    );
    const s = await liveSession();
    render(<Harness />);
    await advance(10);
    say(s.id, "First.", 1000);
    await advance(NOTES_INTERVAL_MS);
    failing = true;
    say(s.id, "Second.", 2000);
    await advance(NOTES_INTERVAL_MS);
    expect(screen.getByRole("alert")).toHaveTextContent("Disk is full.");
    expect(screen.getByText("Notes v1")).toBeInTheDocument();
  });
});

describe("Agenda, recap and action items (PRD MEM-2, MEM-3, MEM-4)", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useNotesStore.setState({ sessionId: null, notes: null, activity: "idle", error: null });
  });
  afterEach(() => vi.restoreAllMocks());

  it("adds, edits and sets agenda items by hand", async () => {
    const s = await liveSession();
    render(<Harness />);
    const agenda = await screen.findByRole("region", { name: "Agenda" });
    fireEvent.click(within(agenda).getByRole("button", { name: "Add agenda" }));
    fireEvent.change(within(agenda).getByLabelText("Agenda items, one per line"), {
      target: { value: "Pricing\n\nRoadmap" },
    });
    fireEvent.click(within(agenda).getByRole("button", { name: "Save agenda" }));
    const list = await within(agenda).findByRole("list", { name: "Agenda items" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((l) => l.textContent),
    ).toEqual([expect.stringContaining("Pricing"), expect.stringContaining("Roadmap")]);
    expect(agenda).toHaveTextContent("0/2 covered");

    fireEvent.change(within(agenda).getByLabelText("Status of Pricing"), {
      target: { value: "covered" },
    });
    await waitFor(() => expect(agenda).toHaveTextContent("1/2 covered"));
    const saved = await mockCommands.getNotes(s.id);
    expect(saved.status === "ok" && saved.data.agenda[0]?.status).toBe("covered");

    fireEvent.click(within(agenda).getByRole("button", { name: "Edit agenda" }));
    expect(within(agenda).getByLabelText("Agenda items, one per line")).toHaveValue(
      "Pricing\nRoadmap",
    );
  });

  it("shows an error when the agenda is too long", async () => {
    await liveSession();
    render(<Harness />);
    const agenda = await screen.findByRole("region", { name: "Agenda" });
    fireEvent.click(within(agenda).getByRole("button", { name: "Add agenda" }));
    fireEvent.change(within(agenda).getByLabelText("Agenda items, one per line"), {
      target: { value: "x".repeat(201) },
    });
    fireEvent.click(within(agenda).getByRole("button", { name: "Save agenda" }));
    expect(await within(agenda).findByRole("alert")).toHaveTextContent("up to 30 items");
  });

  it("'What did I miss?' recaps only the chosen minutes", async () => {
    const calls: LlmRequest[] = [];
    vi.spyOn(mockCommands, "llmStream").mockImplementation((requestId, _p, req) => {
      calls.push(req);
      queueMicrotask(() => {
        mockAnswerBus.emitToken({ requestId, delta: "- Priya asked **you** for the quote" });
        mockAnswerBus.emitDone({
          requestId,
          usage: { inputTokens: 1, outputTokens: 1 },
          finishReason: "end_turn",
        });
      });
      return Promise.resolve({ status: "ok", data: null });
    });
    const s = await liveSession();
    say(s.id, "Ancient history from the start.", 60_000);
    say(s.id, "Can you send the quote?", 700_000);
    render(<Harness />);
    fireEvent.change(await screen.findByLabelText("Recap period"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "What did I miss?" }));
    const card = await screen.findByRole("region", { name: "What you missed" });
    expect(card).toHaveTextContent("Last 2 minutes");
    expect(card).toHaveTextContent("Priya asked you for the quote");
    expect(calls[0]?.messages[0]?.content).toContain("Can you send the quote?");
    expect(calls[0]?.messages[0]?.content).not.toContain("Ancient history");
    fireEvent.click(within(card).getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("region", { name: "What you missed" })).toBeNull();
  });

  it("the recap button waits for the session to start", async () => {
    const s = await mockCommands.createSession("Draft");
    if (s.status !== "ok") throw new Error("create");
    useSessionStore.setState({ session: s.data });
    render(<Harness />);
    await screen.findByRole("region", { name: "Agenda" });
    expect(screen.queryByRole("button", { name: "What did I miss?" })).toBeNull();
  });

  it("action items can be ticked off", async () => {
    const s = await liveSession();
    await mockCommands.saveSummary(s.id, "Recap", [
      { text: "Send the quote", owner: "Priya", due: "Monday" },
    ]);
    render(<Harness />);
    const box = await screen.findByRole("checkbox", { name: /Send the quote/ });
    expect(box).not.toBeChecked();
    fireEvent.click(box);
    await waitFor(() => expect(box).toBeChecked());
    const saved = await mockCommands.getNotes(s.id);
    expect(saved.status === "ok" && saved.data.actionItems[0]?.done).toBe(true);
  });

  it("has no accessibility violations (axe)", async () => {
    const s = await liveSession();
    await mockCommands.setAgenda(s.id, ["Pricing", "Roadmap"]);
    await mockCommands.saveSummary(s.id, "Recap", [{ text: "Send it", owner: null, due: null }]);
    const { container } = render(<Harness />);
    await screen.findByRole("list", { name: "Agenda items" });
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});
