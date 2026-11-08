/** Quick actions on an attached screenshot (PRD SCR-5). */
export const SCREENSHOT_ACTIONS = [
  {
    kind: "explain",
    label: "Explain",
    prompt: "Explain what is on this screenshot and what it means in the context of the meeting.",
  },
  {
    kind: "summarise",
    label: "Summarise",
    prompt: "Summarise the key points on this screenshot in 3 to 5 short bullets.",
  },
  {
    kind: "table",
    label: "Extract table",
    prompt:
      'Extract the table (or tabular data) on this screenshot as a Markdown table. Copy the values exactly; write "unclear" for anything you can\'t read.',
  },
  {
    kind: "answer",
    label: "Answer question on screen",
    prompt:
      "There is a question on this screenshot. Quote it, then answer it using the meeting and the documents.",
  },
  {
    kind: "debug",
    label: "Debug this code",
    prompt:
      "Debug the code or error on this screenshot: say what is wrong and why, then give the fix as a code block.",
  },
] as const;

export type ScreenshotActionKind = (typeof SCREENSHOT_ACTIONS)[number]["kind"];

export function screenshotAction(kind: ScreenshotActionKind) {
  return SCREENSHOT_ACTIONS.find((a) => a.kind === kind) ?? SCREENSHOT_ACTIONS[0];
}
