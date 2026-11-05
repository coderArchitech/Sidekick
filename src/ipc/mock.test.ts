import { beforeEach, describe, expect, it } from "vitest";
import { DEFAULT_SETTINGS, mockCommands, resetMocks } from "./mock";

describe("mock settings commands", () => {
  beforeEach(resetMocks);

  it("starts with the Rust defaults", async () => {
    const result = await mockCommands.getSettings();
    expect(result).toEqual({ status: "ok", data: DEFAULT_SETTINGS });
  });

  it("applies partial patches and keeps other sections", async () => {
    await mockCommands.updateSettings({ autoAnswer: false });
    const result = await mockCommands.getSettings();
    expect(result.status === "ok" && result.data.autoAnswer).toBe(false);
    expect(result.status === "ok" && result.data.stt).toEqual(DEFAULT_SETTINGS.stt);
  });

  it("treats an absent search field as unchanged and null as cleared", async () => {
    await mockCommands.updateSettings({ search: { provider: "tavily", model: null } });
    await mockCommands.updateSettings({ autoAnswer: true });
    let result = await mockCommands.getSettings();
    expect(result.status === "ok" && result.data.search?.provider).toBe("tavily");
    await mockCommands.updateSettings({ search: null });
    result = await mockCommands.getSettings();
    expect(result.status === "ok" && result.data.search).toBeNull();
  });
});
