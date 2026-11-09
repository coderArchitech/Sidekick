import { act, render } from "@testing-library/react";
import { beforeEach, describe, expect, it } from "vitest";
import { mockCommands, resetMocks } from "@/ipc/testing";
import { applyTheme, resolveTheme, useAppTheme } from "./theme";

function Harness() {
  useAppTheme();
  return null;
}

const wait = (ms: number) => act(() => new Promise((r) => setTimeout(r, ms)));

describe("themes (PRD UI-8)", () => {
  beforeEach(() => {
    resetMocks();
    delete document.documentElement.dataset.theme;
  });

  it("resolves System from the OS preference", () => {
    expect(resolveTheme("system", true)).toBe("light");
    expect(resolveTheme("system", false)).toBe("dark");
    expect(resolveTheme("high_contrast", true)).toBe("high-contrast");
    expect(resolveTheme("light", false)).toBe("light");
    expect(resolveTheme("dark", true)).toBe("dark");
  });

  it("sets data-theme on <html>", () => {
    applyTheme("light");
    expect(document.documentElement.dataset.theme).toBe("light");
    applyTheme("high_contrast");
    expect(document.documentElement.dataset.theme).toBe("high-contrast");
  });

  it("follows Settings, also when another window changes them", async () => {
    await mockCommands.updateSettings({ theme: "light" });
    render(<Harness />);
    await wait(10);
    expect(document.documentElement.dataset.theme).toBe("light");
    // Saving settings (e.g. in the Settings window) emits settings-changed.
    await act(async () => {
      await mockCommands.updateSettings({ theme: "high_contrast" });
    });
    expect(document.documentElement.dataset.theme).toBe("high-contrast");
  });
});
