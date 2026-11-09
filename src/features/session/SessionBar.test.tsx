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

describe("Auto-answer toggle", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
  });

  it("shows the session's setting and toggles it", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    const auto = screen.getByRole("button", { name: "Auto" });
    expect(auto).toHaveAttribute("aria-pressed", "true"); // default on
    await user.click(auto);
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "false"),
    );
    const s = useSessionStore.getState().session;
    const stored = await mockCommands.listSessions(null, null);
    expect(
      stored.status === "ok" && stored.data.items.find((x) => x.id === s?.id)?.autoAnswer,
    ).toBe(false);
  });

  it("picks the session's Skill from enabled Skills", async () => {
    const user = userEvent.setup();
    await mockCommands.setSkillEnabled("teacher", false);
    render(<SessionBar />);
    await createSession(user);
    const picker = screen.getByRole("combobox", { name: "Skill" });
    await waitFor(() => expect(within(picker).getAllByRole("option")).toHaveLength(5));
    expect(within(picker).queryByRole("option", { name: "Teacher" })).toBeNull();
    await user.selectOptions(picker, "sales-discovery");
    await waitFor(() =>
      expect(useSessionStore.getState().session?.skillId).toBe("sales-discovery"),
    );
    await user.selectOptions(picker, "");
    await waitFor(() => expect(useSessionStore.getState().session?.skillId).toBeNull());
  });

  it("uses more Skills at once alongside the main one (SKL-5)", async () => {
    const user = userEvent.setup();
    render(<SessionBar />);
    await createSession(user);
    // No main Skill yet: nothing to add to.
    expect(screen.queryByRole("button", { name: /More Skills/ })).toBeNull();
    const picker = screen.getByRole("combobox", { name: "Skill" });
    await waitFor(() => expect(within(picker).getAllByRole("option").length).toBeGreaterThan(2));
    await user.selectOptions(picker, "teacher");

    await user.click(await screen.findByRole("button", { name: /More Skills/ }));
    const dialog = screen.getByRole("dialog", { name: "Also use" });
    expect(within(dialog).queryByRole("checkbox", { name: "Teacher" })).toBeNull(); // the main one
    await user.click(within(dialog).getByRole("checkbox", { name: "Sales Discovery" }));
    await user.click(within(dialog).getByRole("checkbox", { name: "Interview Coach" }));
    await waitFor(() =>
      expect(useSessionStore.getState().session?.extraSkillIds).toEqual([
        "sales-discovery",
        "interview-coach",
      ]),
    );
    // Two extra at most: the rest are disabled.
    expect(within(dialog).getByRole("checkbox", { name: "Negotiation Coach" })).toBeDisabled();
    expect(screen.getByRole("button", { name: /More Skills/ })).toHaveTextContent("+2");

    // Switching the main Skill to an extra one moves it.
    await user.keyboard("{Escape}");
    await user.selectOptions(picker, "sales-discovery");
    await waitFor(() =>
      expect(useSessionStore.getState().session?.extraSkillIds).toEqual(["interview-coach"]),
    );
  });

  async function endedSession(user: ReturnType<typeof userEvent.setup>) {
    render(<SessionBar />);
    await createSession(user, "Retro");
    await user.click(screen.getByRole("button", { name: "Start" }));
    await user.click(screen.getByLabelText("Transcribe me only"));
    await user.click(screen.getByRole("button", { name: "Start listening" }));
    await user.click(await screen.findByRole("button", { name: "End" }));
    return screen.findByRole("combobox", { name: "Keep this session" });
  }

  it("after the session ends, chooses how long to keep it", async () => {
    const user = userEvent.setup();
    const keep = await endedSession(user);
    expect(keep).toHaveValue("30d");
    await user.selectOptions(keep, "forever");
    await waitFor(() => expect(useSessionStore.getState().session?.retention).toBe("forever"));
  });

  it("deletes an ended session after confirming", async () => {
    const user = userEvent.setup();
    await endedSession(user);
    const id = useSessionStore.getState().session?.id ?? "";
    await user.click(screen.getByRole("button", { name: "Delete session…" }));
    expect(
      screen.getByText(/Delete transcript, notes, answers and documents\?/),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(useSessionStore.getState().session).toBeNull());
    const left = await mockCommands.listSessions(null, null);
    expect(left.status === "ok" && left.data.items.some((s) => s.id === id)).toBe(false);
  });
});
