import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import axe from "axe-core";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import { Overlay } from "./Overlay";
import { mockCommands, mockWindowCalls, resetMocks } from "@/ipc/testing";
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
    expect(screen.getByRole("tabpanel")).toHaveTextContent("Start a session to keep notes.");
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

describe("Overlay appearance (PRD UI-1, UI-3)", () => {
  beforeEach(() => {
    resetMocks();
    useOverlayStore.setState({ tab: "answers", opacity: 0.9, fontSize: 14, compact: false });
    document.documentElement.style.fontSize = "";
  });

  const savedOverlay = async () => {
    await new Promise((r) => setTimeout(r, 350)); // opacity saves after it settles
    const s = await mockCommands.getSettings();
    if (s.status !== "ok") throw new Error("settings");
    return s.data.overlay;
  };

  it("loads the saved appearance", async () => {
    await mockCommands.setOverlay({ opacity: 55, fontSize: 18, compact: true });
    render(<Overlay />);
    await waitFor(() => expect(useOverlayStore.getState().fontSize).toBe(18));
    expect(useOverlayStore.getState().opacity).toBe(0.55);
    await userEvent.click(screen.getByRole("button", { name: "View" }));
    expect(screen.getByRole("switch", { name: "Compact" })).toBeChecked();
  });

  it("text size buttons scale the text and are saved", async () => {
    const user = userEvent.setup();
    render(<Overlay />);
    await waitFor(() => expect(document.documentElement.style.fontSize).toBe("16px"));
    await user.click(screen.getByRole("button", { name: "View" }));
    expect(screen.getByRole("dialog", { name: "View options" })).toHaveTextContent("14px");
    await user.click(screen.getByRole("button", { name: "Larger text" }));
    await user.click(screen.getByRole("button", { name: "Larger text" }));
    expect(document.documentElement.style.fontSize).toBe(`${(16 * 16) / 14}px`);
    expect((await savedOverlay()).fontSize).toBe(16);
    for (let i = 0; i < 10; i++)
      await user.click(screen.getByRole("button", { name: "Smaller text" }));
    expect(useOverlayStore.getState().fontSize).toBe(12);
    expect(screen.getByRole("button", { name: "Smaller text" })).toBeDisabled();
  });

  it("compact mode hides the session bar and documents, and is saved", async () => {
    const user = userEvent.setup();
    render(<Overlay />);
    expect(screen.getByRole("button", { name: "New session" })).toBeInTheDocument();
    await user.click(screen.getByRole("button", { name: "View" }));
    await user.click(screen.getByRole("switch", { name: "Compact" }));
    expect(screen.queryByRole("button", { name: "New session" })).not.toBeInTheDocument();
    expect(screen.queryByText(/Documents/)).not.toBeInTheDocument();
    expect(screen.getAllByRole("tab")).toHaveLength(4);
    expect((await savedOverlay()).compact).toBe(true);
  });

  it("opacity is saved as a percentage once it settles", async () => {
    render(<Overlay />);
    await userEvent.click(screen.getByRole("button", { name: "View" }));
    const slider = screen.getByRole("slider", { name: "Panel opacity" });
    fireEvent.change(slider, { target: { value: "0.5" } });
    fireEvent.change(slider, { target: { value: "0.45" } });
    expect((await savedOverlay()).opacity).toBe(45);
  });

  it("the View menu closes on Escape and outside clicks", async () => {
    const user = userEvent.setup();
    render(<Overlay />);
    await user.click(screen.getByRole("button", { name: "View" }));
    expect(screen.getByRole("button", { name: "View" })).toHaveAttribute("aria-expanded", "true");
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("dialog", { name: "View options" })).toBeNull();
    await user.click(screen.getByRole("button", { name: "View" }));
    await user.click(screen.getByRole("tab", { name: "Notes" }));
    expect(screen.queryByRole("dialog", { name: "View options" })).toBeNull();
  });

  it("has no accessibility violations (axe)", async () => {
    const { container } = render(<Overlay />);
    await screen.findByText("v0.0.0-mock");
    const results = await axe.run(container, { rules: { "color-contrast": { enabled: false } } });
    expect(results.violations.map((v) => `${v.id}: ${v.help}`)).toEqual([]);
  });
});
