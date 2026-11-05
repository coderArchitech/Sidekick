import { act, render, screen, within } from "@testing-library/react";
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
