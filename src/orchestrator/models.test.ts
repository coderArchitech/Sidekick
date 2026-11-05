import { describe, expect, it } from "vitest";
import { modelPlan } from "./models";

describe("modelPlan", () => {
  it("uses a small model for cues and the user's choice for full answers", () => {
    expect(modelPlan({ provider: "anthropic", model: null })).toEqual({
      provider: "anthropic",
      full: "claude-opus-5-5",
      cue: "claude-haiku-5-5",
    });
    expect(modelPlan({ provider: "openai", model: "gpt-4o" })).toEqual({
      provider: "openai",
      full: "gpt-4o",
      cue: "gpt-5-mini",
    });
    expect(modelPlan({ provider: "gemini", model: null }).cue).toBe("gemini-2.5-flash");
  });

  it("local models use the chosen (or first installed) model for both", () => {
    expect(modelPlan({ provider: "ollama", model: "llava" })).toEqual({
      provider: "ollama",
      full: "llava",
      cue: "llava",
    });
    expect(modelPlan({ provider: "ollama", model: null }).full).toBe("default");
  });
});
