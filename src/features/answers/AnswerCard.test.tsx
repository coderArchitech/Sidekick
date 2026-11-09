import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { resetMocks, savedAnswers } from "@/ipc/testing";
import { useAnswersStore, type AnswerView } from "@/orchestrator";
import { AnswerCard } from "./AnswerCard";
import { LiveAnswers } from "./LiveAnswers";

// Shiki is covered by its own lazy import in the real app; tests use the plain fallback.
vi.mock("./highlight", () => ({ highlight: () => Promise.resolve(null) }));

function view(over: Partial<AnswerView> = {}): AnswerView {
  return {
    id: "q1",
    question: "What is our refund window?",
    directed: false,
    source: "detected",
    cue: "30 days from purchase.",
    body: "Refunds are accepted within **30 days** [1].",
    sources: [{ n: 1, type: "doc", ref: "c1", label: "policy.pdf p.2" }],
    status: "done",
    error: null,
    answerId: null,
    ...over,
  };
}

describe("AnswerCard", () => {
  beforeEach(() => {
    resetMocks();
    useAnswersStore.getState().clear();
  });

  it("shows the cue first, expands to the full answer, then sources", async () => {
    const user = userEvent.setup();
    render(<AnswerCard view={view()} />);
    expect(screen.getByText("30 days from purchase.")).toBeInTheDocument();
    expect(screen.queryByText(/Refunds are accepted/)).not.toBeInTheDocument();
    expect(screen.queryByRole("list", { name: "Sources" })).not.toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "▸ Full answer" }));
    const body = screen.getByText(/Refunds are accepted/);
    const sources = screen.getByRole("list", { name: "Sources" });
    expect(within(sources).getByText("[1] policy.pdf p.2")).toBeInTheDocument();
    // Sources come after the body.
    expect(body.compareDocumentPosition(sources) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();

    await user.click(screen.getByRole("button", { name: "▾ Hide full answer" }));
    expect(screen.queryByText(/Refunds are accepted/)).not.toBeInTheDocument();
  });

  it("opens the full answer by default when the question is for you", () => {
    render(<AnswerCard view={view({ directed: true })} />);
    expect(screen.getByText("For you")).toBeInTheDocument();
    expect(screen.getByText(/Refunds are accepted/)).toBeInTheDocument();
  });

  it("copies code only when the copy button is clicked (UI-4c)", async () => {
    const user = userEvent.setup();
    const write = vi.spyOn(navigator.clipboard, "writeText");
    render(
      <AnswerCard
        view={view({ directed: true, body: "Use:\n\n```ts\nconst x = 1;\n```\n\nThen run it." })}
      />,
    );
    expect(screen.getByText("const x = 1;")).toBeInTheDocument();
    expect(write).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Copy code" }));
    expect(write).toHaveBeenCalledTimes(1);
    expect(write).toHaveBeenCalledWith("const x = 1;");
    expect(await screen.findByRole("button", { name: "Copied" })).toBeInTheDocument();
  });

  it("never renders raw HTML or script from the model", () => {
    const { container } = render(
      <AnswerCard
        view={view({
          directed: true,
          body: 'Hi <script>window.hacked = 1</script> <img src=x onerror="alert(1)"> <b>bold</b>',
        })}
      />,
    );
    expect(container.querySelector("script")).toBeNull();
    expect(container.querySelector("img")).toBeNull();
    expect(container.querySelector("b")).toBeNull();
    expect((window as unknown as { hacked?: number }).hacked).toBeUndefined();
  });

  it("does not turn links into clickable anchors", () => {
    const { container } = render(
      <AnswerCard view={view({ directed: true, body: "See [docs](https://evil.example)." })} />,
    );
    expect(container.querySelector("a")).toBeNull();
    expect(screen.getByText("docs")).toHaveAttribute("title", "https://evil.example");
  });

  it("rating calls rate_answer with the right value", async () => {
    const user = userEvent.setup();
    savedAnswers.push({
      id: "a1",
      answer: {} as (typeof savedAnswers)[number]["answer"],
      rating: null,
      createdAt: 0,
    });
    render(<AnswerCard view={view({ answerId: "a1" })} />);

    await user.click(screen.getByRole("button", { name: "Helpful" }));
    expect(savedAnswers[0]?.rating).toBe(1);
    expect(screen.getByRole("button", { name: "Helpful" })).toHaveAttribute("aria-pressed", "true");

    await user.click(screen.getByRole("button", { name: "Not helpful" }));
    expect(savedAnswers[0]?.rating).toBe(-1);

    await user.click(screen.getByRole("button", { name: "Not helpful" }));
    expect(savedAnswers[0]?.rating).toBeNull();
    expect(screen.getByRole("button", { name: "Not helpful" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
  });

  it("shows errors and stopped answers", () => {
    const { rerender } = render(
      <AnswerCard view={view({ status: "error", cue: "", body: "", error: "Key rejected" })} />,
    );
    expect(screen.getByRole("alert")).toHaveTextContent("Key rejected");
    rerender(<AnswerCard view={view({ status: "cancelled", cue: "", body: "" })} />);
    expect(screen.getByText("Stopped.")).toBeInTheDocument();
  });
});

describe("LiveAnswers", () => {
  beforeEach(() => useAnswersStore.getState().clear());

  it("asks for a session when there are no answers", () => {
    render(<LiveAnswers />);
    expect(
      screen.getByText("Start a session and answers to detected questions appear here."),
    ).toBeInTheDocument();
  });

  it("renders streamed tokens progressively, newest answer first", () => {
    render(<LiveAnswers />);
    const upsert = useAnswersStore.getState().upsert;
    act(() => upsert(view({ status: "thinking", cue: "", body: "", sources: [] })));
    expect(screen.getByText("Thinking…")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Stop" })).toBeInTheDocument();

    act(() => upsert(view({ status: "streaming", cue: "30 days", body: "", sources: [] })));
    expect(screen.getByText("30 days")).toBeInTheDocument();
    act(() => upsert(view({ status: "streaming", cue: "30 days from purchase.", body: "" })));
    expect(screen.getByText("30 days from purchase.")).toBeInTheDocument();

    act(() => upsert(view()));
    expect(screen.queryByRole("button", { name: "Stop" })).not.toBeInTheDocument();

    act(() => upsert(view({ id: "q2", question: "Who owns billing?", cue: "Priya." })));
    const cards = screen.getAllByRole("article");
    expect(cards.map((c) => c.getAttribute("aria-label"))).toEqual([
      "Answer to: Who owns billing?",
      "Answer to: What is our refund window?",
    ]);
  });
});

describe("AnswerCard follow-up chips (PRD QA-9)", () => {
  beforeEach(resetMocks);

  it("shows the four chips on a finished answer only", () => {
    const { rerender } = render(<AnswerCard view={view({ status: "streaming" })} />);
    expect(screen.queryByRole("group", { name: "Follow-ups" })).toBeNull();
    rerender(<AnswerCard view={view()} />);
    const chips = screen.getByRole("group", { name: "Follow-ups" });
    expect(within(chips).getAllByRole("button")).toHaveLength(4);
  });

  it("without a live session a chip says to start one", async () => {
    const user = userEvent.setup();
    render(<AnswerCard view={view()} />);
    await user.click(screen.getByRole("button", { name: "Shorter" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Start a session");
  });
});

describe("AnswerCard tools (PRD SKL-4)", () => {
  beforeEach(resetMocks);

  it("lists the tools an agent Skill used, with failures", () => {
    render(
      <AnswerCard
        view={view({
          tools: [
            { tool: "calculator", input: "2400 * 15%", output: "360", ok: true },
            {
              tool: "web_search",
              input: "fed rate",
              output: "Web search isn't set up.",
              ok: false,
            },
          ],
        })}
      />,
    );
    const list = screen.getByRole("list", { name: "Tools used" });
    expect(
      within(list)
        .getAllByRole("listitem")
        .map((l) => l.textContent),
    ).toEqual(["Calculator: 2400 * 15% = 360", "Web search: fed rate (Web search isn't set up.)"]);
  });

  it("says it's checking while the agent step runs", () => {
    render(<AnswerCard view={view({ status: "thinking", cue: "", body: "", tools: [] })} />);
    expect(screen.getByText("Checking whether tools help…")).toBeInTheDocument();
  });
});
