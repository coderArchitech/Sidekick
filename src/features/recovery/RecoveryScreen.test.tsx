import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { StartupProblem } from "@/ipc";
import { mockFiles, mockSkills, mockStartup, resetMocks } from "@/ipc/testing";
import { RecoveryScreen } from "./RecoveryScreen";

const base: StartupProblem = {
  kind: "key_missing",
  message: "The key that unlocks Sidekick's data is missing from the keychain.",
  dataDir: "/Users/me/Library/Application Support/com.sidekick.app",
  hasBackup: false,
  keychainName: "Keychain",
};

function show(p: Partial<StartupProblem> = {}) {
  const problem = { ...base, ...p };
  mockStartup.problem = problem;
  render(<RecoveryScreen problem={problem} />);
  return userEvent.setup();
}

describe("RecoveryScreen", () => {
  beforeEach(() => resetMocks());

  it("explains a missing key and starts fresh only after confirming", async () => {
    const user = show();
    expect(
      screen.getByRole("heading", { name: "Sidekick can't unlock your saved data" }),
    ).toBeInTheDocument();
    expect(screen.getByText(/missing from Keychain/)).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Start fresh…" }));
    expect(mockStartup.actions).toEqual([]);
    const confirm = screen.getByRole("group", { name: "Confirm start fresh" });
    expect(confirm).toHaveTextContent(base.dataDir);

    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(screen.queryByRole("group", { name: "Confirm start fresh" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Start fresh…" }));
    await user.click(screen.getByRole("button", { name: "Move it aside and start fresh" }));
    expect(mockStartup.actions).toEqual(["startFresh"]);
    expect(screen.getByRole("status")).toHaveTextContent("Restarting Sidekick…");
  });

  it("only offers Try again when the keychain is locked: the data is fine", async () => {
    const user = show({ kind: "keychain_unavailable", keychainName: "Credential Manager" });
    expect(screen.getByText(/Credential Manager is locked/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Start fresh…" })).not.toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(mockStartup.actions).toEqual(["retry"]);
  });

  it("puts Restore backup first when an unreadable database has one", async () => {
    const user = show({ kind: "database_unreadable", hasBackup: true });
    const buttons = screen
      .getAllByRole("button")
      .map((b) => b.textContent)
      .filter((t) => t !== "Details");
    expect(buttons).toEqual(["Restore backup", "Start fresh…", "Try again", "Export logs…"]);
    expect(screen.getByRole("button", { name: "Restore backup" })).toHaveClass("btn-primary");
    await user.click(screen.getByRole("button", { name: "Restore backup" }));
    expect(mockStartup.actions).toEqual(["restoreBackup"]);
  });

  it("doesn't offer a backup that doesn't exist", () => {
    show({ kind: "database_unreadable", hasBackup: false });
    expect(screen.queryByRole("button", { name: "Restore backup" })).not.toBeInTheDocument();
  });

  it("exports the logs, and does nothing when the save dialog is cancelled", async () => {
    const user = show({ kind: "other", message: "Couldn't create the app data folder." });
    await user.click(screen.getByRole("button", { name: "Export logs…" }));
    expect(screen.queryByText(/Saved the log bundle/)).not.toBeInTheDocument();

    mockFiles.saveAs = ["/tmp/sidekick-logs.zip"];
    await user.click(screen.getByRole("button", { name: "Export logs…" }));
    expect(await screen.findByText("Saved the log bundle for a bug report.")).toBeInTheDocument();
    expect(mockSkills.files.has("/tmp/sidekick-logs.zip")).toBe(true);
  });

  it("shows a failed action's error and lets you try again", async () => {
    const user = show();
    mockStartup.problem = null; // the core refuses: nothing to recover
    await user.click(screen.getByRole("button", { name: "Try again" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("nothing to recover");
    expect(screen.getByRole("button", { name: "Try again" })).toBeEnabled();
  });

  it("keeps the technical details behind a disclosure", () => {
    show();
    expect(screen.getByText("Details")).toBeInTheDocument();
    expect(screen.getByText(base.message)).not.toBeVisible();
  });
});
