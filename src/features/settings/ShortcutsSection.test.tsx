import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, mockCommands, resetMocks } from "@/ipc/testing";
import { SettingsWindow } from "./SettingsWindow";

describe("Shortcuts section", () => {
  beforeEach(resetMocks);

  it("lists every action with its default", async () => {
    render(<SettingsWindow />);
    expect(await screen.findByRole("textbox", { name: "Show / hide overlay" })).toHaveValue(
      "CmdOrCtrl+Shift+H",
    );
    expect(screen.getByRole("textbox", { name: "Bookmark this moment" })).toHaveValue(
      "CmdOrCtrl+Shift+B",
    );
    expect(screen.getByRole("button", { name: "Save shortcuts" })).toBeDisabled();
  });

  it("saves a changed shortcut", async () => {
    const user = userEvent.setup();
    render(<SettingsWindow />);
    const input = await screen.findByRole("textbox", { name: "Bookmark this moment" });
    await user.clear(input);
    await user.type(input, "Alt+Shift+M");
    await user.click(screen.getByRole("button", { name: "Save shortcuts" }));
    expect(await screen.findByText("Shortcuts saved.")).toBeInTheDocument();
    const s = await mockCommands.getSettings();
    expect(s.status === "ok" && s.data.shortcuts.find((b) => b.action === "bookmark")?.keys).toBe(
      "Alt+Shift+M",
    );
  });

  it("shows why a duplicate is rejected and keeps the old set", async () => {
    const user = userEvent.setup();
    render(<SettingsWindow />);
    const input = await screen.findByRole("textbox", { name: "Bookmark this moment" });
    await user.clear(input);
    await user.type(input, "CmdOrCtrl+Shift+H");
    await user.click(screen.getByRole("button", { name: "Save shortcuts" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("used twice");
    const s = await mockCommands.getSettings();
    expect(s.status === "ok" && s.data.shortcuts).toEqual(DEFAULT_SETTINGS.shortcuts);
  });

  it("undo restores the saved values", async () => {
    const user = userEvent.setup();
    render(<SettingsWindow />);
    const input = await screen.findByRole("textbox", { name: "Screenshot" });
    await user.type(input, "X");
    await user.click(screen.getByRole("button", { name: "Undo changes" }));
    expect(input).toHaveValue("CmdOrCtrl+Shift+S");
  });
});
