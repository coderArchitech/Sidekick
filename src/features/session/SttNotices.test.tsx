import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, mockSessionBus, resetMocks } from "@/ipc/testing";
import { SessionBar } from "./SessionBar";
import { useSessionStore } from "./store";
import { sttNotice } from "./SttNotices";

async function liveSession() {
  const s = await mockCommands.createSession("Standup", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session, warnings: [], stt: {} });
  return started.data.session;
}

describe("speech-to-text notices", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], stt: {}, error: null, busy: false });
  });

  it("explains a silent microphone with the permission path", () => {
    const n = sttNotice("mic", "silent");
    expect(n?.alert).toBe(true);
    expect(n?.text).toMatch(/isn't hearing your microphone/);
    expect(n?.text).toMatch(/Privacy/);
  });

  it("says which stream failed, and stays quiet when all is well", () => {
    expect(sttNotice("system", "failed")?.text).toMatch(/stopped for other participants/);
    expect(sttNotice("mic", "reconnecting")).toEqual({
      alert: false,
      text: "Reconnecting speech-to-text for your microphone…",
    });
    expect(sttNotice("mic", "live")).toBeNull();
    expect(sttNotice("mic", undefined)).toBeNull();
  });

  it("shows status events for the live session in the session bar, and clears them", async () => {
    const session = await liveSession();
    render(<SessionBar />);
    await act(async () => {
      await Promise.resolve();
      mockSessionBus.emit("stt", { sessionId: session.id, stream: "mic", state: "silent" });
    });
    expect(screen.getByRole("alert")).toHaveTextContent("isn't hearing your microphone");

    act(() => mockSessionBus.emit("stt", { sessionId: session.id, stream: "mic", state: "live" }));
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });

  it("ignores events for another session", async () => {
    await liveSession();
    render(<SessionBar />);
    await act(async () => {
      await Promise.resolve();
      mockSessionBus.emit("stt", { sessionId: "other", stream: "mic", state: "failed" });
    });
    expect(screen.queryByRole("alert")).not.toBeInTheDocument();
  });
});
