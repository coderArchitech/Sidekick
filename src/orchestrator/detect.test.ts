import { describe, expect, it } from "vitest";
import { findName, isQuestion, levenshtein, shouldAnswer, similar } from "./detect";
import { LABELLED } from "./questions.fixture";

describe("isQuestion on the labelled set", () => {
  it("meets the PRD targets: precision above 85%, recall above 80%", () => {
    const names = ["Nitesh", "Priya", "Arjun", "Meera", "Rahul"];
    let tp = 0;
    let fp = 0;
    let fn = 0;
    const mistakes: string[] = [];
    for (const [text, expected] of LABELLED) {
      const got = isQuestion(text, names);
      if (got && expected) tp++;
      else if (got && !expected) {
        fp++;
        mistakes.push(`FP: ${text}`);
      } else if (!got && expected) {
        fn++;
        mistakes.push(`FN: ${text}`);
      }
    }
    const precision = tp / (tp + fp);
    const recall = tp / (tp + fn);
    console.info(
      `precision ${(precision * 100).toFixed(1)}%, recall ${(recall * 100).toFixed(1)}%\n${mistakes.join("\n")}`,
    );
    expect(LABELLED.length).toBeGreaterThanOrEqual(200);
    expect(precision).toBeGreaterThan(0.85);
    expect(recall).toBeGreaterThan(0.8);
  });
});

describe("names", () => {
  it("matches names with small STT misspellings", () => {
    expect(similar("Nitesh", "nitesh")).toBe(true);
    expect(similar("Nitesh", "nitish")).toBe(true);
    expect(similar("Priya", "pria")).toBe(true);
    expect(similar("Alexandra", "alexandre")).toBe(true);
  });

  it("doesn't trigger on similar ordinary words", () => {
    expect(similar("Priya", "prior")).toBe(false);
    expect(similar("Nitesh", "finish")).toBe(false);
    expect(similar("Ram", "rum")).toBe(false); // short names must match exactly
    expect(findName("we should finish the prior work", ["Nitesh", "Priya"])).toBeNull();
  });

  it("finds names and nicknames, including possessives and multi-word names", () => {
    expect(findName("Nitish, what's the rate limit?", ["Nitesh", "Nitu"])).toBe("Nitesh");
    expect(findName("over to you Nitu", ["Nitesh", "Nitu"])).toBe("Nitu");
    expect(findName("is that Priya's number?", ["Priya"])).toBe("Priya");
    expect(findName("Mary Ann, can you confirm?", ["Mary Ann"])).toBe("Mary Ann");
    expect(findName("Mary, can you confirm?", ["Mary Ann"])).toBeNull();
    expect(findName("anything", [])).toBeNull();
  });

  it("computes edit distance", () => {
    expect(levenshtein("kitten", "sitting")).toBe(3);
    expect(levenshtein("", "abc")).toBe(3);
  });
});

describe("modes", () => {
  it("directed questions are always answered; others only with Auto-answer", () => {
    expect(shouldAnswer({ directed: true }, false)).toBe(true);
    expect(shouldAnswer({ directed: false }, false)).toBe(false);
    expect(shouldAnswer({ directed: false }, true)).toBe(true);
  });
});

describe("isQuestion on held-out utterances", () => {
  it.each([
    ["round 1 (now tuning data)", "HELD_OUT"],
    ["round 2 (never tuned on)", "HELD_OUT_2"],
  ] as const)("%s meets the targets", async (label, key) => {
    const sets = await import("./heldout.fixture");
    const HELD_OUT = sets[key];
    const names = ["Nitesh", "Priya", "Arjun", "Meera", "Rahul"];
    let tp = 0;
    let fp = 0;
    let fn = 0;
    const mistakes: string[] = [];
    for (const [text, expected] of HELD_OUT) {
      const got = isQuestion(text, names);
      if (got && expected) tp++;
      else if (got && !expected) {
        fp++;
        mistakes.push(`FP: ${text}`);
      } else if (!got && expected) {
        fn++;
        mistakes.push(`FN: ${text}`);
      }
    }
    const precision = tp / (tp + fp);
    const recall = tp / (tp + fn);
    console.info(
      `${label}: precision ${(precision * 100).toFixed(1)}%, recall ${(recall * 100).toFixed(1)}%\n${mistakes.join("\n")}`,
    );
    // Recorded, not tuned: the bar here is the PRD target, checked honestly.
    expect(precision).toBeGreaterThan(0.85);
    expect(recall).toBeGreaterThan(0.8);
  });
});
