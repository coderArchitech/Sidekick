import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { SettingsWindow } from "./SettingsWindow";

describe("Profile section", () => {
  beforeEach(resetMocks);

  it("adds, removes and saves nicknames with the name", async () => {
    const user = userEvent.setup();
    render(<SettingsWindow />);
    await user.type(await screen.findByLabelText("Your name"), "Nitesh");
    await user.type(screen.getByLabelText("New nickname"), "Nitu{Enter}");
    await user.type(screen.getByLabelText("New nickname"), "NK");
    await user.click(screen.getByRole("button", { name: "Add" }));
    await user.type(screen.getByLabelText("New nickname"), "nitu{Enter}"); // duplicate ignored
    const list = screen.getByRole("list", { name: "Nicknames" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((li) => li.firstChild?.textContent),
    ).toEqual(["Nitu", "NK"]);

    await user.click(within(list).getByRole("button", { name: "Remove NK" }));
    await user.click(screen.getByRole("button", { name: "Save profile" }));
    expect(await screen.findByText("Profile saved.")).toBeInTheDocument();
    const s = await mockCommands.getSettings();
    expect(s.status === "ok" && s.data.profile).toEqual({ name: "Nitesh", nicknames: ["Nitu"] });
  });

  it("shows 'None yet' without nicknames and disables Add for blank input", async () => {
    render(<SettingsWindow />);
    expect(await screen.findByText("None yet.")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Add" })).toBeDisabled();
  });
});
