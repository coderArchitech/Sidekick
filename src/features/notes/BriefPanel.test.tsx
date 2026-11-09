import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { act } from "react";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { BriefPanel } from "./BriefPanel";

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function session() {
  const s = await mockCommands.createSession("Call");
  if (s.status !== "ok") throw new Error("create");
  return s.data.id;
}

describe("Pre-meeting brief (PRD CTX-4)", () => {
  beforeEach(resetMocks);

  it("without documents says what to add", async () => {
    const user = userEvent.setup();
    render(<BriefPanel sessionId={await session()} />);
    await user.click(screen.getByRole("button", { name: "Write brief" }));
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Add documents or turn on a Knowledge Pack first.",
    );
  });

  it("writes from session documents and attached packs, saves, and reloads", async () => {
    const user = userEvent.setup();
    const id = await session();
    await mockCommands.ingestDocument(id, "/docs/agenda.md");
    const pack = await mockCommands.createPack("Sales kit");
    if (pack.status !== "ok") throw new Error("pack");
    await mockCommands.ingestPackDocument(pack.data.id, "/docs/prices.md");
    await mockCommands.setSessionPacks(id, [pack.data.id]);
    await wait(200);

    const { unmount } = render(<BriefPanel sessionId={id} />);
    await user.click(screen.getByRole("button", { name: "Write brief" }));
    const panel = screen.getByRole("region", { name: "Pre-meeting brief" });
    // The mock model echoes the documents it was sent.
    expect(await screen.findByText(/Mock content of agenda\.md/)).toBeInTheDocument();
    expect(panel).toHaveTextContent("Mock answer to:");
    expect(screen.getByRole("button", { name: "Rewrite" })).toBeInTheDocument();
    const saved = await mockCommands.getBrief(id);
    const md = saved.status === "ok" ? (saved.data?.contentMd ?? "") : "";
    expect(md).toContain("Mock content of agenda.md");
    expect(md).toContain('<document name="prices.md" pack="Sales kit">');

    unmount();
    render(<BriefPanel sessionId={id} />);
    const toggle = await screen.findByRole("button", { name: /Pre-meeting brief/ });
    expect(toggle).toHaveAttribute("aria-expanded", "false");
    await user.click(toggle);
    expect(screen.getByText(/Mock content of agenda\.md/)).toBeInTheDocument();
  });
});
