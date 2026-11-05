import { create } from "zustand";
import { commands, type ChatMessage, type SavedAnswer } from "@/ipc";
import type { AnswerView } from "./answerer";

/** Turns of history sent with a follow-up (about 3 exchanges keeps prompts small). */
export const CHAT_HISTORY_TURNS = 3;

interface ChatState {
  sessionId: string | null;
  /** Oldest first. */
  turns: AnswerView[];
  /** Bumped by the Ask hotkey; the Chat input focuses when it changes. */
  focusTick: number;
  upsert: (v: AnswerView) => void;
  /** Loads saved chat for a session (PRD QA-8: history per session). */
  load: (sessionId: string | null) => Promise<void>;
  requestFocus: () => void;
}

function fromSaved(a: SavedAnswer): AnswerView {
  return {
    id: `saved:${a.answerId}`,
    question: a.question,
    directed: a.directed,
    source: a.source,
    cue: a.cue ?? "",
    body: a.bodyMd,
    sources: a.sources.map((s, i) => ({
      n: i + 1,
      type: s.type === "transcript" ? "transcript" : "doc",
      ref: s.ref,
      label: s.label,
    })),
    status: "done",
    error: null,
    answerId: a.answerId,
  };
}

export const useChatStore = create<ChatState>((set, get) => ({
  sessionId: null,
  turns: [],
  focusTick: 0,
  upsert: (v) =>
    set((s) => {
      const i = s.turns.findIndex((x) => x.id === v.id);
      if (i === -1) return { turns: [...s.turns, v] };
      const turns = s.turns.slice();
      turns[i] = v;
      return { turns };
    }),
  load: async (sessionId) => {
    set({ sessionId, turns: [] });
    if (!sessionId) return;
    const r = await commands.listAnswers(sessionId, "chat");
    if (r.status !== "ok" || get().sessionId !== sessionId) return;
    // Keep anything asked while loading.
    set((s) => ({ turns: [...r.data.map(fromSaved), ...s.turns] }));
  },
  requestFocus: () => set((s) => ({ focusTick: s.focusTick + 1 })),
}));

/**
 * Previous finished exchanges as model messages, so follow-ups like "and the
 * burst limit?" make sense (PRD QA-8). Only answered turns count.
 */
export function chatHistory(turns: AnswerView[], max = CHAT_HISTORY_TURNS): ChatMessage[] {
  return turns
    .filter((t) => t.status === "done" && t.body.trim() !== "")
    .slice(-max)
    .flatMap((t): ChatMessage[] => [
      { role: "user", content: t.question },
      { role: "assistant", content: t.body },
    ]);
}
