import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { Overlay } from "./Overlay";
import { mockWindowCalls, resetMocks } from "@/ipc/testing";
import { useOverlayStore } from "./store";

describe("Overlay", () => {
  beforeEach(() => useOverlayStore.setState({ tab: "answers", opacity: 0.9 }));

  it("shows the four tabs with Live Answers selected", () => {
    render(<Overlay />);
    const tabs = screen.getAllByRole("tab");
    expect(tabs.map((t) => t.textContent)).toEqual(["Live Answers", "Transcript", "Notes", "Chat"]);
    expect(screen.getByRole("tab", { name: "Live Answers" })).toHaveAttribute(
      "aria-selected",
      "true",
    );
  });

  it("switches tab on click", async () => {
    render(<Overlay />);
    await userEvent.click(screen.getByRole("tab", { name: "Notes" }));
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Notes will appear here.");
  });

  it("shows the app version from the Rust core (mocked)", async () => {
    render(<Overlay />);
    expect(await screen.findByText("v0.0.0-mock")).toBeInTheDocument();
  });
});

describe("Overlay settings button", () => {
  beforeEach(resetMocks);

  it("opens the Settings window", async () => {
    render(<Overlay />);
    await userEvent.click(screen.getByRole("button", { name: "Settings" }));
    expect(mockWindowCalls.openSettings).toBe(1);
  });
});

describe("overlay store", () => {
  it("clamps opacity to 20%-100% (PRD UI-1)", () => {
    useOverlayStore.getState().setOpacity(0.05);
    expect(useOverlayStore.getState().opacity).toBe(0.2);
    useOverlayStore.getState().setOpacity(3);
    expect(useOverlayStore.getState().opacity).toBe(1);
  });
});
