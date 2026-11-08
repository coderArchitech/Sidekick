import type { ChatMessage } from "@/ipc";
import type { AnswerView } from "./answerer";

/** The follow-up chips under a finished answer (PRD QA-9). */
export const FOLLOW_UPS = [
  {
    kind: "shorter",
    label: "Shorter",
    instruction: "Give the same answer again in at most two short sentences.",
  },
  {
    kind: "detail",
    label: "More detail",
    instruction: "Expand on the answer with more detail, specifics and numbers from the sources.",
  },
  {
    kind: "example",
    label: "Give an example",
    instruction: "Give one concrete, realistic example that illustrates the answer.",
  },
  {
    kind: "simple",
    label: "Explain simply",
    instruction:
      "Explain the answer simply, as to someone new to the topic: plain words, no jargon, a short analogy if it helps.",
  },
] as const;

export type FollowUpKind = (typeof FOLLOW_UPS)[number]["kind"];

export interface FollowUpInput {
  /** Sent to the model: the instruction, tied to the original question. */
  text: string;
  /** Shown and saved, e.g. "Shorter: What's the rate limit?". */
  label: string;
  /** Document search uses the original question, not the instruction. */
  query: string;
  /** The original exchange, so the full answer can rework it. */
  history: ChatMessage[];
}

/** Whether `view` can take follow-ups: finished with an answer. */
export function canFollowUp(view: AnswerView): boolean {
  return view.status === "done" && (view.body || view.cue).trim() !== "";
}

export function followUpInput(view: AnswerView, kind: FollowUpKind): FollowUpInput {
  const f = FOLLOW_UPS.find((x) => x.kind === kind) ?? FOLLOW_UPS[0];
  // A chip on a follow-up answer refers back to the first question.
  const original = view.question.replace(
    /^(Shorter|More detail|Give an example|Explain simply): /,
    "",
  );
  return {
    text: `About "${original}": ${f.instruction}`,
    label: `${f.label}: ${original}`,
    query: original,
    history: [
      { role: "user", content: original },
      { role: "assistant", content: (view.body || view.cue).trim() },
    ],
  };
}
