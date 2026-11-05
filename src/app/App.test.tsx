import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { currentWindow } from "@/ipc";
import { App } from "./App";

describe("App window routing", () => {
  afterEach(() => window.history.replaceState(null, "", "/"));

  it("reads the window from the query", () => {
    expect(currentWindow("?window=settings")).toBe("settings");
    expect(currentWindow("")).toBe("overlay");
    expect(currentWindow("?window=unknown")).toBe("overlay");
  });

  it("renders the overlay by default and Settings for ?window=settings", async () => {
    const { unmount } = render(<App />);
    expect(screen.getByRole("tablist")).toBeInTheDocument();
    unmount();
    window.history.replaceState(null, "", "/?window=settings");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });
});
