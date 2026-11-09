import { describe, expect, it } from "vitest";
import { isQuestion } from "./detect";
import { INTERVIEW } from "./interview.fixture";

describe("interview prompts (the other person's side of a technical interview)", () => {
  it("detects tasks and questions, not interviewer chit-chat", () => {
    let tp = 0;
    let fp = 0;
    let fn = 0;
    const mistakes: string[] = [];
    for (const [text, label] of INTERVIEW) {
      const got = isQuestion(text);
      if (got && label) tp++;
      else if (got && !label) {
        fp++;
        mistakes.push(`false positive: ${text}`);
      } else if (!got && label) {
        fn++;
        mistakes.push(`missed: ${text}`);
      }
    }
    const precision = tp / (tp + fp);
    const recall = tp / (tp + fn);
    console.info(
      `interview: precision ${(precision * 100).toFixed(1)}%, recall ${(recall * 100).toFixed(1)}%\n${mistakes.join("\n")}`,
    );
    expect(precision).toBeGreaterThanOrEqual(0.95);
    expect(recall).toBeGreaterThanOrEqual(0.95);
  });
});
