import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { useSessionStore } from "@/features/session";
import { mockCommands, mockFiles, mockSessionBus, resetMocks } from "@/ipc/testing";
import { Library } from "./Library";
import { useLibraryStore } from "./store";

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

async function draftSession() {
  const s = await mockCommands.createSession("Library", null);
  if (s.status !== "ok") throw new Error("create");
  useSessionStore.setState({ session: s.data });
  return s.data;
}

async function openPanel(user: ReturnType<typeof userEvent.setup>) {
  await user.click(await screen.findByRole("button", { name: /Documents/ }));
}

describe("Library", () => {
  beforeEach(() => {
    resetMocks();
    useSessionStore.setState({ session: null, warnings: [], error: null, busy: false });
    useLibraryStore.setState({
      sessionId: null,
      documents: [],
      percent: {},
      sources: {},
      error: null,
    });
  });

  it("is hidden without a session", () => {
    const { container } = render(<Library />);
    expect(container).toBeEmptyDOMElement();
  });

  it("adds picked files and shows progress until ready", async () => {
    const user = userEvent.setup();
    await draftSession();
    render(<Library />);
    await openPanel(user);
    mockFiles.picked = ["/home/me/spec.pdf", "/home/me/deck.pptx"];
    await user.click(screen.getByRole("button", { name: "Add files" }));
    const list = await screen.findByRole("list", { name: "Documents" });
    expect(within(list).getAllByRole("listitem")).toHaveLength(2);
    // Wait for both mocked imports to finish, not a fixed time: a busy CI
    // runner can be slower than the mock's few timer steps.
    await waitFor(() => expect(within(list).getAllByText(/Ready/)).toHaveLength(2));
    expect(screen.getByRole("button", { name: /Documents \(2\/2 ready\)/ })).toBeInTheDocument();
  });

  it("shows why a file failed and retries it", async () => {
    const user = userEvent.setup();
    await draftSession();
    render(<Library />);
    await openPanel(user);
    mockFiles.picked = ["/tmp/broken.pdf"];
    await user.click(screen.getByRole("button", { name: "Add files" }));
    await wait(60);
    expect(
      await screen.findByText("broken.pdf: Couldn't read broken.pdf: the PDF is damaged."),
    ).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "Retry" }));
    await wait(60);
    expect(
      within(screen.getByRole("list", { name: "Documents" })).getAllByRole("listitem"),
    ).toHaveLength(1);
  });

  it("rejects unsupported files with the reason", async () => {
    const user = userEvent.setup();
    await draftSession();
    render(<Library />);
    await openPanel(user);
    mockFiles.picked = ["/tmp/archive.zip"];
    await user.click(screen.getByRole("button", { name: "Add files" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("can't read archive.zip");
  });

  it("adds links and refuses non-web links", async () => {
    const user = userEvent.setup();
    await draftSession();
    render(<Library />);
    await openPanel(user);
    await user.type(screen.getByLabelText("Link to add"), "ftp://example.com/x");
    await user.click(screen.getByRole("button", { name: "Add link" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Only http and https");
    await user.clear(screen.getByLabelText("Link to add"));
    await user.type(screen.getByLabelText("Link to add"), "https://example.com/limits{Enter}");
    expect(await screen.findByText("https://example.com/limits")).toBeInTheDocument();
    expect(screen.getByLabelText("Link to add")).toHaveValue("");
  });

  it("removes documents", async () => {
    const user = userEvent.setup();
    const s = await draftSession();
    await mockCommands.ingestDocument(s.id, "/tmp/notes.md");
    render(<Library />);
    await openPanel(user);
    await user.click(await screen.findByRole("button", { name: "Remove notes.md" }));
    expect(screen.queryByText("notes.md")).not.toBeInTheDocument();
    const left = await mockCommands.listDocuments(s.id);
    expect(left.status === "ok" && left.data).toEqual([]);
  });

  it("ingests files dropped onto the window", async () => {
    await draftSession();
    render(<Library />);
    await wait(10);
    mockSessionBus.emit("drop", ["/Users/me/Agenda.docx"]);
    expect(await screen.findByText("Agenda.docx")).toBeInTheDocument();
  });
});
