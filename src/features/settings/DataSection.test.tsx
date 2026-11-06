import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
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
