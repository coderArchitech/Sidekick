import { describe, expect, it } from "vitest";
import type { AnswerView } from "./answerer";
import { chatHistory } from "./chatStore";

const turn = (n: number, over: Partial<AnswerView> = {}): AnswerView => ({
  id: `t${n}`,
  question: `q${n}`,
  directed: true,
  source: "chat",
  cue: "",
  body: `a${n}`,
  sources: [],
  status: "done",
  error: null,
  answerId: null,
  ...over,
});

describe("chatHistory", () => {
  it("keeps the last finished exchanges as user/assistant pairs", () => {
    const h = chatHistory([turn(1), turn(2), turn(3), turn(4)], 2);
    expect(h).toEqual([
      { role: "user", content: "q3" },
      { role: "assistant", content: "a3" },
      { role: "user", content: "q4" },
      { role: "assistant", content: "a4" },
    ]);
  });

  it("skips unfinished, failed and empty turns", () => {
    const h = chatHistory([
      turn(1, { status: "streaming" }),
      turn(2, { status: "error", body: "" }),
      turn(3, { status: "cancelled" }),
      turn(4, { body: "  " }),
      turn(5),
    ]);
    expect(h).toEqual([
      { role: "user", content: "q5" },
      { role: "assistant", content: "a5" },
    ]);
  });
});
