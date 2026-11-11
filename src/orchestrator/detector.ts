import { findName, isQuestion } from "./detect";

export interface SegmentIn {
  id: number;
  speakerLabel: string;
  isUser: boolean;
  text: string;
  endMs: number;
}

export interface DetectedQuestion {
  text: string;
  /** Addressed to the user by name or nickname (QA-2). */
  directed: boolean;
  matchedName: string | null;
  speakerLabel: string;
  segmentIds: number[];
  endMs: number;
  /** When the detector decided, for latency metrics. */
  detectedAt: number;
}

export interface DetectorOptions {
  names: string[];
  /** Wait this long after a segment ends, so a question split across segments is read whole (tech doc 5.5). */
  debounceMs?: number;
  /** The shorter wait after a segment that ends with "?": the speaker has finished asking. */
  questionDebounceMs?: number;
  onQuestion: (q: DetectedQuestion) => void;
}

/**
 * Turns finished transcript segments into questions (PRD QA-1, QA-2).
 * - The user's own speech never triggers answers, and it ends the other
 *   speaker's turn at once (they stopped talking).
 * - Consecutive segments from the same speaker within the debounce are read
 *   together, so "What's the rate limit" + "for the Pro tier?" is one question.
 * - A segment that ends with "?" is decided after 250 ms instead of 600 ms.
 */
export class QuestionDetector {
  private names: string[];
  private readonly debounceMs: number;
  private readonly questionDebounceMs: number;
  private readonly onQuestion: (q: DetectedQuestion) => void;
  private pending: { speakerLabel: string; segs: SegmentIn[] } | null = null;
  private timer: ReturnType<typeof setTimeout> | null = null;

  constructor(opts: DetectorOptions) {
    this.names = opts.names;
    this.debounceMs = opts.debounceMs ?? 600;
    this.questionDebounceMs = opts.questionDebounceMs ?? 250;
    this.onQuestion = opts.onQuestion;
  }

  setNames(names: string[]) {
    this.names = names;
  }

  push(seg: SegmentIn) {
    if (seg.isUser) {
      this.flush();
      return;
    }
    if (this.pending && this.pending.speakerLabel !== seg.speakerLabel) this.flush();
    this.pending ??= { speakerLabel: seg.speakerLabel, segs: [] };
    this.pending.segs.push(seg);
    if (this.timer) clearTimeout(this.timer);
    // A line ending in "?" is a finished question: decide almost at once, but
    // still leave a moment for an immediate follow-on ("…for the Pro tier?").
    const wait = seg.text.trim().endsWith("?") ? this.questionDebounceMs : this.debounceMs;
    this.timer = setTimeout(() => this.flush(), wait);
  }

  /** Decides on what's pending now. */
  flush() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    const p = this.pending;
    this.pending = null;
    if (!p) return;
    const text = p.segs.map((s) => s.text.trim()).join(" ");
    // Judge the last sentence: "Thanks. And what's the rate limit?" is a question.
    const sentences = text.split(/(?<=[.!?])\s+/).filter(Boolean);
    const last = sentences[sentences.length - 1] ?? text;
    const matchedName = findName(text, this.names);
    const question = isQuestion(last, this.names) || isQuestion(text, this.names);
    if (!question) return;
    this.onQuestion({
      text,
      directed: matchedName !== null,
      matchedName,
      speakerLabel: p.speakerLabel,
      segmentIds: p.segs.map((s) => s.id),
      endMs: Math.max(...p.segs.map((s) => s.endMs)),
      detectedAt: Date.now(),
    });
  }

  dispose() {
    if (this.timer) clearTimeout(this.timer);
    this.timer = null;
    this.pending = null;
  }
}
