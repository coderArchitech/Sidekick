import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { QuestionDetector, type DetectedQuestion, type SegmentIn } from "./detector";

let id = 0;
const seg = (text: string, speakerLabel = "speaker_1", isUser = false): SegmentIn => ({
  id: ++id,
  speakerLabel,
  isUser,
  text,
  endMs: id * 1000,
});

describe("QuestionDetector", () => {
  let found: DetectedQuestion[];
  let d: QuestionDetector;

  beforeEach(() => {
    vi.useFakeTimers();
    found = [];
    d = new QuestionDetector({ names: ["Nitesh", "Nitu"], onQuestion: (q) => found.push(q) });
  });
  afterEach(() => vi.useRealTimers());

  it("decides 250 ms after a line ending in '?', and 600 ms after any other line", () => {
    d.push(seg("What's the rate limit for the Pro tier?"));
    vi.advanceTimersByTime(249);
    expect(found).toHaveLength(0);
    vi.advanceTimersByTime(1);
    expect(found).toHaveLength(1);
    expect(found[0]?.directed).toBe(false);

    d.push(seg("Walk me through how you would design a URL shortener"));
    vi.advanceTimersByTime(599);
    expect(found).toHaveLength(1);
    vi.advanceTimersByTime(1);
    expect(found).toHaveLength(2);
  });

  it("merges a question split across segments of the same speaker", () => {
    d.push(seg("So what's the rate limit"));
    vi.advanceTimersByTime(300);
    d.push(seg("for the Pro tier?"));
    vi.advanceTimersByTime(600);
    expect(found.map((q) => q.text)).toEqual(["So what's the rate limit for the Pro tier?"]);
    expect(found[0]?.segmentIds).toHaveLength(2);
  });

  it("flags directed questions, including misspelled names", () => {
    d.push(seg("Nitish, can you confirm the SLA?"));
    vi.advanceTimersByTime(600);
    expect(found[0]).toMatchObject({ directed: true, matchedName: "Nitesh" });
  });

  it("ignores statements and the user's own speech", () => {
    d.push(seg("The rate limit is 600 per minute."));
    d.push(seg("What should I say?", "me", true));
    vi.advanceTimersByTime(1000);
    expect(found).toHaveLength(0);
  });

  it("the user starting to talk ends the other speaker's turn immediately", () => {
    d.push(seg("Any blockers on your side?"));
    d.push(seg("Let me think.", "me", true));
    expect(found).toHaveLength(1);
  });

  it("a different speaker ends the previous one's turn", () => {
    d.push(seg("Can you send the deck?", "speaker_1"));
    d.push(seg("Yes, I'll do that.", "speaker_2"));
    expect(found.map((q) => q.speakerLabel)).toEqual(["speaker_1"]);
  });

  it("finds a question at the end of a longer turn", () => {
    d.push(seg("Thanks for the overview. Really helpful. And how do refunds work?"));
    vi.advanceTimersByTime(600);
    expect(found).toHaveLength(1);
  });

  it("names can change while running; dispose drops pending work", () => {
    d.setNames(["Priya"]);
    d.push(seg("Priya, the numbers for March please."));
    vi.advanceTimersByTime(600);
    expect(found[0]?.matchedName).toBe("Priya");
    d.push(seg("What's next?"));
    d.dispose();
    vi.advanceTimersByTime(1000);
    expect(found).toHaveLength(1);
  });
});

describe("the other person's interview prompts (bug: tasks weren't answered)", () => {
  it("a task from the other speaker is a question; room lines count as the other person", () => {
    vi.useFakeTimers();
    const found: string[] = [];
    const d = new QuestionDetector({ names: [], onQuestion: (q) => found.push(q.text) });
    d.push({ id: 1, speakerLabel: "speaker_1", isUser: false, text: "Okay so", endMs: 1000 });
    d.push({
      id: 2,
      speakerLabel: "speaker_1",
      isUser: false,
      text: "design a URL shortener like bitly",
      endMs: 2000,
    });
    vi.advanceTimersByTime(700);
    d.push({
      id: 3,
      speakerLabel: "room",
      isUser: false,
      text: "Implement debounce in JavaScript.",
      endMs: 5000,
    });
    vi.advanceTimersByTime(700);
    d.push({
      id: 4,
      speakerLabel: "me",
      isUser: true,
      text: "Design a URL shortener",
      endMs: 6000,
    });
    vi.advanceTimersByTime(700);
    expect(found).toEqual([
      "Okay so design a URL shortener like bitly",
      "Implement debounce in JavaScript.",
    ]);
    vi.useRealTimers();
  });
});
