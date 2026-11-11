import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useSessionStore } from "@/features/session";
import { mockCommands, mockWindowCalls, resetMocks } from "@/ipc/testing";
import { useNotesStore } from "@/orchestrator";
import { QuitButton, waitForSummary } from "./QuitButton";

const tick = () => new Promise((r) => setTimeout(r, 0));

async function liveSession() {
  const s = await mockCommands.createSession("Design review", null);
  if (s.status !== "ok") throw new Error("create");
  await mockCommands.confirmConsent(s.data.id, "participants_agreed");
  const started = await mockCommands.startSession(s.data.id);
  if (started.status !== "ok") throw new Error("start");
  useSessionStore.setState({ session: started.data.session });
}

describe("QuitButton", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], stt: {}, error: null, busy: false });
    useNotesStore.setState({ activity: "idle" });
  });

  it("is a red button that quits at once when nothing is listening", async () => {
    const user = userEvent.setup();
    render(<QuitButton />);
    const button = screen.getByRole("button", { name: "Quit Sidekick" });
    expect(button).toHaveClass("btn-danger");
    await user.click(button);
    expect(mockWindowCalls.quit).toBe(1);
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("asks first during a live session, and Cancel keeps listening", async () => {
    await liveSession();
    const user = userEvent.setup();
    render(<QuitButton />);
    await user.click(screen.getByRole("button", { name: "Quit Sidekick" }));
    const dialog = screen.getByRole("dialog", { name: "Quit Sidekick?" });
    expect(dialog).toHaveTextContent("“Design review” is still listening");
    expect(screen.getByRole("button", { name: "Cancel" })).toHaveFocus();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(mockWindowCalls.quit).toBe(0);
    expect(useSessionStore.getState().session?.status).toBe("live");
  });

  it("ends the session, waits for the summary, then quits", async () => {
    await liveSession();
    const user = userEvent.setup();
    render(<QuitButton />);
    await user.click(screen.getByRole("button", { name: "Quit Sidekick" }));
    await user.click(screen.getByRole("button", { name: "End session and quit" }));
    expect(useSessionStore.getState().session?.status).toBe("ended");
    expect(screen.getByRole("status")).toHaveTextContent("writing its summary");
    // The overlay writes the summary (useNotes); quitting waits for it.
    act(() => useNotesStore.setState({ activity: "summarizing" }));
    await act(tick);
    expect(mockWindowCalls.quit).toBe(0);
    act(() => useNotesStore.setState({ activity: "idle" }));
    await act(tick);
    expect(mockWindowCalls.quit).toBe(1);
  });

  it("Quit now skips waiting for the summary", async () => {
    await liveSession();
    const user = userEvent.setup();
    render(<QuitButton />);
    await user.click(screen.getByRole("button", { name: "Quit Sidekick" }));
    await user.click(screen.getByRole("button", { name: "End session and quit" }));
    act(() => useNotesStore.setState({ activity: "summarizing" }));
    await user.click(screen.getByRole("button", { name: "Quit now" }));
    expect(mockWindowCalls.quit).toBe(1);
  });
});

describe("waitForSummary", () => {
  beforeEach(() => useNotesStore.setState({ activity: "idle" }));

  it("returns soon when no summary starts", async () => {
    const t = Date.now();
    await waitForSummary(5_000, 30);
    expect(Date.now() - t).toBeLessThan(1_000);
  });

  it("gives up after the timeout if the summary never finishes", async () => {
    useNotesStore.setState({ activity: "summarizing" });
    const t = Date.now();
    await waitForSummary(60, 10);
    expect(Date.now() - t).toBeGreaterThanOrEqual(50);
  });
});
