import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import axe from "axe-core";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { PackPicker } from "./PackPicker";
import { PacksSection } from "./PacksSection";

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

describe("Settings → Knowledge Packs (PRD CTX-5)", () => {
  beforeEach(resetMocks);

  it("creates a pack, adds a link, renames and deletes it", async () => {
    const user = userEvent.setup();
    render(<PacksSection />);
    expect(await screen.findByText("No packs yet.")).toBeInTheDocument();
    await user.type(screen.getByLabelText("New pack name"), "Product specs");
    await user.click(screen.getByRole("button", { name: "Create pack" }));

    const list = await screen.findByRole("list", { name: "Knowledge Packs" });
    expect(within(list).getByRole("button", { name: /Product specs/ })).toHaveAttribute(
      "aria-expanded",
      "true",
    );
    await user.type(
      screen.getByLabelText("Link to add to Product specs"),
      "https://example.com/spec",
    );
    await user.click(screen.getByRole("button", { name: "Add link" }));
    await wait(200); // the mock ingest finishes
    const docs = await screen.findByRole("list", { name: "Documents in Product specs" });
    expect(docs).toHaveTextContent("https://example.com/spec");
    await waitFor(() => expect(list).toHaveTextContent("1/1 ready"));

    await user.click(screen.getByRole("button", { name: "Rename" }));
    const nameBox = screen.getByLabelText("New name for Product specs");
    await user.clear(nameBox);
    await user.type(nameBox, "Specs v2");
    await user.click(screen.getByRole("button", { name: "Save name" }));
    expect(await within(list).findByRole("button", { name: /Specs v2/ })).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete pack" }));
    await user.click(screen.getByRole("button", { name: "Delete Specs v2" }));
    expect(await screen.findByText("No packs yet.")).toBeInTheDocument();
  });

  it("shows errors: blank name is blocked, bad links are refused", async () => {
    const user = userEvent.setup();
    render(<PacksSection />);
    expect(screen.getByRole("button", { name: "Create pack" })).toBeDisabled();
    await user.type(screen.getByLabelText("New pack name"), "Kit");
    await user.click(screen.getByRole("button", { name: "Create pack" }));
    await user.type(await screen.findByLabelText("Link to add to Kit"), "ftp://nope");
    await user.click(screen.getByRole("button", { name: "Add link" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Only http and https");
  });

  it("has no accessibility violations (axe)", async () => {
    await mockCommands.createPack("Specs");
    const { container } = render(<PacksSection />);
    await screen.findByRole("list", { name: "Knowledge Packs" });
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});

describe("Overlay pack picker (PRD CTX-5)", () => {
  beforeEach(resetMocks);

  async function session() {
    const s = await mockCommands.createSession("Call");
    if (s.status !== "ok") throw new Error("create");
    return s.data.id;
  }

  it("points to Settings when there are no packs", async () => {
    render(<PackPicker sessionId={await session()} />);
    expect(await screen.findByText(/Create packs in Settings/)).toBeInTheDocument();
  });

  it("turning a pack on makes answers search its documents", async () => {
    const id = await session();
    const pack = await mockCommands.createPack("Sales kit");
    if (pack.status !== "ok") throw new Error("pack");
    await mockCommands.ingestPackDocument(pack.data.id, "/docs/prices.md");
    await wait(200);
    render(<PackPicker sessionId={id} />);
    const box = await screen.findByRole("checkbox", { name: /Sales kit/ });
    const before = await mockCommands.retrieveContext(id, "price", null);
    expect(before.status === "ok" && before.data.chunks).toEqual([]);

    fireEvent.click(box);
    await waitFor(() => expect(box).toBeChecked());
    expect(await mockCommands.listSessionPacks(id)).toEqual({ status: "ok", data: [pack.data.id] });
    const after = await mockCommands.retrieveContext(id, "price", null);
    expect(after.status === "ok" && after.data.chunks.map((c) => c.documentName)).toEqual([
      "prices.md",
    ]);

    fireEvent.click(box);
    await waitFor(() => expect(box).not.toBeChecked());
  });
});
