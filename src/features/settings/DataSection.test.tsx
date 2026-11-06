import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, mockFiles, mockSkills, resetMocks } from "@/ipc/testing";
import { DataSection } from "./DataSection";

describe("Settings → Your data", () => {
  beforeEach(() => resetMocks());

  it("deletes every session only after typing DELETE", async () => {
    const user = userEvent.setup();
    await mockCommands.createSession("One");
    await mockCommands.createSession("Two");
    render(<DataSection />);
    await user.click(screen.getByRole("button", { name: "Delete everything…" }));
    const go = screen.getByRole("button", { name: "Delete all meeting data" });
    expect(go).toBeDisabled();
    await user.type(screen.getByLabelText("Type DELETE to confirm"), "delete");
    expect(go).toBeDisabled();
    await user.clear(screen.getByLabelText("Type DELETE to confirm"));
    await user.type(screen.getByLabelText("Type DELETE to confirm"), "DELETE");
    await user.click(go);
    expect(await screen.findByRole("status")).toHaveTextContent("Deleted 2 sessions");
    const left = await mockCommands.listSessions(null, null);
    expect(left.status === "ok" && left.data.items).toEqual([]);
  });

  it("cancel deletes nothing", async () => {
    const user = userEvent.setup();
    await mockCommands.createSession("Keep me");
    render(<DataSection />);
    await user.click(screen.getByRole("button", { name: "Delete everything…" }));
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    const left = await mockCommands.listSessions(null, null);
    expect(left.status === "ok" && left.data.items).toHaveLength(1);
  });
});

describe("Settings → Diagnostics export", () => {
  beforeEach(() => resetMocks());

  it("exports the log bundle to the chosen file", async () => {
    const { DiagnosticsPanel } = await import("./DiagnosticsPanel");
    const user = userEvent.setup();
    mockFiles.saveAs.push("/tmp/sidekick-logs.zip");
    render(<DiagnosticsPanel />);
    await user.click(screen.getByRole("button", { name: "Export logs…" }));
    expect(await screen.findByRole("status")).toHaveTextContent(
      "Saved the log bundle (1 log file and diagnostics).",
    );
    expect(mockSkills.files.has("/tmp/sidekick-logs.zip")).toBe(true);
  });

  it("does nothing when the save dialog is cancelled", async () => {
    const { DiagnosticsPanel } = await import("./DiagnosticsPanel");
    const user = userEvent.setup();
    render(<DiagnosticsPanel />);
    await user.click(screen.getByRole("button", { name: "Export logs…" }));
    expect(screen.queryByRole("status")).toBeNull();
  });
});
