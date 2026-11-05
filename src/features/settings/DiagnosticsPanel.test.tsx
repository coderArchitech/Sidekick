import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { DiagnosticsPanel } from "./DiagnosticsPanel";

describe("DiagnosticsPanel", () => {
  beforeEach(resetMocks);

  it("shows every stage with no data at first", async () => {
    render(<DiagnosticsPanel />);
    const rows = await screen.findAllByRole("row");
    expect(rows).toHaveLength(6); // header + 5 stages
    expect(screen.getAllByText("No data")).toHaveLength(5);
  });

  it("shows percentiles and health, and refreshes", async () => {
    const user = userEvent.setup();
    await mockCommands.recordLatency("question_to_cue", 1200);
    await mockCommands.recordLatency("retrieval", 400);
    render(<DiagnosticsPanel />);
    const cue = await screen.findByRole("row", { name: /question to cue/i });
    expect(within(cue).getAllByText("1200 ms")).toHaveLength(2);
    expect(within(cue).getByText("Healthy")).toBeInTheDocument();
    expect(
      within(screen.getByRole("row", { name: /context retrieval/i })).getByText("Slow"),
    ).toBeInTheDocument();

    await mockCommands.recordLatency("llm_first_token", 500);
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    const llm = await screen.findByRole("row", { name: /llm first token/i });
    expect(await within(llm).findByText("Healthy")).toBeInTheDocument();
  });
});
