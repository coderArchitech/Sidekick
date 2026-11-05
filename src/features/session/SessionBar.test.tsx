import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCalls, mockCommands, resetMocks } from "@/ipc/testing";
import { SessionBar } from "./SessionBar";
import { useSessionStore } from "./store";

async function createSession(user: ReturnType<typeof userEvent.setup>, name = "Acme QBR") {
  await user.type(screen.getByLabelText("Session name"), name);
  await user.click(screen.getByRole("button", { name: "New session" }));
  await screen.findByText(name);
}

describe("SessionBar", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
  });

  it("creates a draft session", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    expect(screen.getByRole("button", { name: "New session" })).toBeDisabled();
    await createSession(user);
    expect(screen.getByRole("button", { name: "Start" })).toBeInTheDocument();
  });

  it("asks for consent before listening and needs a choice", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    const dialog = screen.getByRole("dialog", { name: "Before Sidekick listens" });
    expect(within(dialog).getByRole("button", { name: "Start listening" })).toBeDisabled();
    expect(
      within(dialog).getByRole("radio", { name: /participants know and agree/i }),
    ).toHaveFocus();
  });

  it("cancelling consent starts nothing", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
    expect(mockCalls.streams).toEqual([]);
    expect(useSessionStore.getState().session?.status).toBe("draft");
  });

  it("Escape closes the consent dialog", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  });

  it("'Transcribe me only' starts the microphone only", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    await user.click(screen.getByRole("radio", { name: "Transcribe me only" }));
    await user.click(screen.getByRole("button", { name: "Start listening" }));
    expect(await screen.findByText("Listening (you only)")).toBeInTheDocument();
    expect(mockCalls.streams).toEqual(["mic"]);
    expect(useSessionStore.getState().session?.consent).toBe("me_only");
  });

  it("participants agreeing starts both streams; pause, resume and end", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    await user.click(screen.getByRole("radio", { name: /participants know and agree/i }));
    await user.click(screen.getByRole("button", { name: "Start listening" }));
    expect(await screen.findByText("Listening")).toBeInTheDocument();
    expect(mockCalls.streams).toEqual(["mic", "system"]);

    await user.click(screen.getByRole("button", { name: "Pause" }));
    expect(await screen.findByText("Paused")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Resume" }));
    expect(await screen.findByText("Listening")).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "End" }));
    expect(await screen.findByText("Session ended.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "New session" })).toBeInTheDocument();
  });

  it("shows errors from the core", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await user.type(screen.getByLabelText("Session name"), "Second meeting");
    // Another session already live: starting is refused.
    const other = await mockCommands.createSession("Other");
    if (other.status === "ok") {
      await mockCommands.confirmConsent(other.data.id, "me_only");
      await mockCommands.startSession(other.data.id);
    }
    await user.click(screen.getByRole("button", { name: "New session" }));
    await user.click(await screen.findByRole("button", { name: "Start" }));
    await user.click(screen.getByRole("radio", { name: "Transcribe me only" }));
    await user.click(screen.getByRole("button", { name: "Start listening" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("End the current session");
  });

  it("the consent dialog has no accessibility violations", async () => {
    const user = userEvent.setup();
    const { container } = render(<SessionBar />);
    await createSession(user);
    await user.click(screen.getByRole("button", { name: "Start" }));
    await waitFor(() => expect(screen.getByRole("dialog")).toBeInTheDocument());
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});

describe("mock start without consent", () => {
  beforeEach(resetMocks);

  it("returns CONSENT_REQUIRED", async () => {
    const s = await mockCommands.createSession("No consent");
    if (s.status !== "ok") throw new Error("create failed");
    const started = await mockCommands.startSession(s.data.id);
    expect(started.status === "error" && started.error.code).toBe("CONSENT_REQUIRED");
  });
});
