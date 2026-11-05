import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useSessionStore } from "@/features/session";
import type { Session } from "@/ipc";
import { mockCommands, mockSessionBus, mockTranscriptLine, resetMocks } from "@/ipc/testing";
import { clock, displayName } from "./store";
import { TranscriptView } from "./TranscriptView";
import { useTranscriptStore } from "./store";

async function liveSession(): Promise<Session> {
  const s = await mockCommands.createSession("Review");
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

describe("TranscriptView", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useTranscriptStore.setState({ sessionId: null, lines: [], partials: {}, bookmarks: [] });
  });

  it("asks for a session first", () => {
    render(<TranscriptView />);
    expect(screen.getByText("Start a session to see the transcript.")).toBeInTheDocument();
  });

  it("loads saved lines in time order", async () => {
    const s = await liveSession();
    mockTranscriptLine(s.id, {
      speakerLabel: "me",
      isUser: true,
      text: "Second",
      startMs: 4000,
      endMs: 5000,
    });
    mockTranscriptLine(s.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "First",
      startMs: 1000,
      endMs: 2000,
    });
    render(<TranscriptView />);
    const list = await screen.findByRole("list", { name: "Transcript" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((li) => li.textContent),
    ).toEqual(["0:01Speaker 1:First", "0:04Me:Second"]);
  });

  it("shows partial text, then replaces it with the final segment", async () => {
    const s = await liveSession();
    render(<TranscriptView />);
    await wait(10); // listeners attached
    mockSessionBus.emit("partial", { sessionId: s.id, stream: "system", text: "what is the" });
    await wait(70);
    expect(screen.getByText("what is the")).toBeInTheDocument();
    mockTranscriptLine(s.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "What is the rate limit?",
      startMs: 0,
      endMs: 900,
    });
    await wait(70);
    expect(screen.queryByText("what is the")).not.toBeInTheDocument();
    expect(screen.getByText("What is the rate limit?")).toBeInTheDocument();
  });

  it("ignores events from other sessions", async () => {
    await liveSession();
    render(<TranscriptView />);
    await wait(10);
    mockSessionBus.emit("partial", { sessionId: "other", stream: "mic", text: "not mine" });
    await wait(70);
    expect(screen.queryByText("not mine")).not.toBeInTheDocument();
  });

  it("renaming a speaker updates all their lines", async () => {
    const user = userEvent.setup();
    const s = await liveSession();
    mockTranscriptLine(s.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "One",
      startMs: 0,
      endMs: 1,
    });
    mockTranscriptLine(s.id, {
      speakerLabel: "speaker_1",
      isUser: false,
      text: "Two",
      startMs: 2,
      endMs: 3,
    });
    render(<TranscriptView />);
    const [first] = await screen.findAllByRole("button", { name: "Speaker 1:" });
    if (!first) throw new Error("no speaker button");
    await user.click(first);
    const input = screen.getByLabelText("New name for Speaker 1");
    await user.clear(input);
    await user.type(input, "Priya{Enter}");
    expect(await screen.findAllByRole("button", { name: "Priya:" })).toHaveLength(2);
    const saved = await mockCommands.getTranscript(s.id);
    expect(saved.status === "ok" && saved.data.lines.map((l) => l.speakerLabel)).toEqual([
      "Priya",
      "Priya",
    ]);
  });

  it("listening indicator follows pause and resume", async () => {
    const s = await liveSession();
    render(<TranscriptView />);
    expect(screen.getByRole("status")).toHaveTextContent("Listening");
    const paused = await mockCommands.pauseSession(s.id);
    if (paused.status === "ok") act(() => useSessionStore.setState({ session: paused.data }));
    expect(screen.getByRole("status")).toHaveTextContent("Paused: not listening");
  });

  it("adds bookmarks", async () => {
    const user = userEvent.setup();
    await liveSession();
    render(<TranscriptView />);
    await user.click(screen.getByRole("button", { name: "Bookmark" }));
    expect(await screen.findByRole("heading", { name: "Bookmarks" })).toBeInTheDocument();
  });
});

describe("transcript helpers", () => {
  it("names speakers", () => {
    expect(displayName("me")).toBe("Me");
    expect(displayName("speaker_3")).toBe("Speaker 3");
    expect(displayName("Priya")).toBe("Priya");
  });

  it("formats clock times", () => {
    expect(clock(0)).toBe("0:00");
    expect(clock(754_000)).toBe("12:34");
  });
});
