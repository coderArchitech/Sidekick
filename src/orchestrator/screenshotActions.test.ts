import { describe, expect, it } from "vitest";
import { SCREENSHOT_ACTIONS, screenshotAction } from "./screenshotActions";

describe("screenshot quick actions (PRD SCR-5)", () => {
  it("offers the five PRD actions, each with its own prompt", () => {
    expect(SCREENSHOT_ACTIONS.map((a) => a.label)).toEqual([
      "Explain",
      "Summarise",
      "Extract table",
      "Answer question on screen",
      "Debug this code",
    ]);
    expect(new Set(SCREENSHOT_ACTIONS.map((a) => a.prompt)).size).toBe(5);
  });

  it("each prompt asks for what its label says", () => {
    expect(screenshotAction("table").prompt).toContain("Markdown table");
    expect(screenshotAction("table").prompt).toContain("unclear");
    expect(screenshotAction("debug").prompt).toContain("fix as a code block");
    expect(screenshotAction("answer").prompt).toContain("Quote it");
    expect(screenshotAction("summarise").prompt).toContain("bullets");
    expect(screenshotAction("explain").prompt).toContain("Explain");
  });
});
