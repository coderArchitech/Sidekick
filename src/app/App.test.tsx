import { render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import { currentWindow } from "@/ipc";
import { mockStartup, resetMocks } from "@/ipc/testing";
import { App } from "./App";

describe("App window routing", () => {
  afterEach(() => {
    window.history.replaceState(null, "", "/");
    resetMocks();
  });

  it("reads the window from the query", () => {
    expect(currentWindow("?window=settings")).toBe("settings");
    expect(currentWindow("")).toBe("overlay");
    expect(currentWindow("?window=unknown")).toBe("overlay");
  });

  it("renders the overlay by default and Settings for ?window=settings", async () => {
    const { unmount } = render(<App />);
    expect(await screen.findByRole("tablist")).toBeInTheDocument();
    unmount();
    window.history.replaceState(null, "", "/?window=settings");
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Settings" })).toBeInTheDocument();
  });

  it("shows the recovery screen instead of the overlay when startup failed", async () => {
    mockStartup.problem = {
      kind: "key_missing",
      message: "The key that unlocks Sidekick's data is missing from the keychain.",
      dataDir: "/data",
      hasBackup: false,
      keychainName: "Keychain",
    };
    const { unmount } = render(<App />);
    expect(
      await screen.findByRole("heading", { name: "Sidekick can't unlock your saved data" }),
    ).toBeInTheDocument();
    expect(screen.queryByRole("tablist")).not.toBeInTheDocument();
    unmount();
    // The Settings window stays empty: nothing in it would work.
    window.history.replaceState(null, "", "/?window=settings");
    const { container } = render(<App />);
    await new Promise((r) => setTimeout(r, 20));
    expect(container).toBeEmptyDOMElement();
  });
});
