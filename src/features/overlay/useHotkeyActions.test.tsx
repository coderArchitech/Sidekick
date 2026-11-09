import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { useSessionStore } from "@/features/session";
import { mockCommands, mockSessionBus, resetMocks } from "@/ipc/testing";
import { useChatStore } from "@/orchestrator";
import { useOverlayStore } from "./store";
import { useHotkeyActions } from "./useHotkeyActions";

function Harness() {
  useHotkeyActions();
  return null;
}

const tick = (ms = 20) => act(() => new Promise((r) => setTimeout(r, ms)));

async function liveSession() {
  const s = await mockCommands.createSession("Hotkeys", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "me_only");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
  return started.data.session;
}

describe("global shortcut actions", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
  });

  it("pause/resume toggles the live session", async () => {
    await liveSession();
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "pause_resume" });
    await tick();
    expect(useSessionStore.getState().session?.status).toBe("paused");
    mockSessionBus.emit("hotkey", { action: "pause_resume" });
    await tick();
    expect(useSessionStore.getState().session?.status).toBe("live");
  });

  it("bookmark adds a bookmark to the running session", async () => {
    const s = await liveSession();
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "bookmark" });
    await tick();
    const t = await mockCommands.getTranscript(s.id);
    expect(t.status === "ok" && t.data.bookmarks).toHaveLength(1);
  });

  it("does nothing without a session, and stops listening on unmount", async () => {
    const { unmount } = render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "pause_resume" });
    await tick();
    expect(useSessionStore.getState().session).toBeNull();
    unmount();
    await liveSession();
    mockSessionBus.emit("hotkey", { action: "pause_resume" });
    await tick();
    expect(useSessionStore.getState().session?.status).toBe("live");
  });
});

describe("auto-answer shortcut", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
  });

  it("toggles Auto-answer for the session", async () => {
    await liveSession();
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "auto_answer" });
    await tick();
    expect(useSessionStore.getState().session?.autoAnswer).toBe(false);
  });

  it("Ask opens the Chat tab and focuses its input", async () => {
    useOverlayStore.setState({ tab: "answers" });
    const before = useChatStore.getState().focusTick;
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "ask" });
    await tick();
    expect(useOverlayStore.getState().tab).toBe("chat");
    expect(useChatStore.getState().focusTick).toBe(before + 1);
  });

  it("Screenshot captures the screen and opens Chat to ask about it", async () => {
    useOverlayStore.setState({ tab: "answers" });
    useChatStore.setState({ pending: null, captureError: null });
    await liveSession();
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "screenshot" });
    await tick();
    expect(useOverlayStore.getState().tab).toBe("chat");
    expect(useChatStore.getState().pending).toMatchObject({ width: 1568, height: 882 });
  });

  it("Screenshot does nothing without a session", async () => {
    useChatStore.setState({ pending: null });
    render(<Harness />);
    await tick();
    mockSessionBus.emit("hotkey", { action: "screenshot" });
    await tick();
    expect(useChatStore.getState().pending).toBeNull();
  });

  it("Switch Skill cycles enabled Skills, then none", async () => {
    const s = await liveSession();
    render(<Harness />);
    await tick();
    const ids: (string | null)[] = [];
    for (let i = 0; i < 6; i++) {
      mockSessionBus.emit("hotkey", { action: "switch_skill" });
      await tick(40);
      ids.push(useSessionStore.getState().session?.skillId ?? null);
    }
    expect(ids).toEqual([
      "interview-coach",
      "negotiation-coach",
      "sales-discovery",
      "teacher",
      "technical-architect",
      null,
    ]);
    expect(s.id).toBeTruthy();
  });
});
