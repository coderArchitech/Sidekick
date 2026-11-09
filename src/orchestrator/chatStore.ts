import { create } from "zustand";
import {
  commands,
  type CaptureRequest,
  type ChatMessage,
  type SavedAnswer,
  type Screenshot,
} from "@/ipc";
import type { AnswerView } from "./answerer";

/** Turns of history sent with a follow-up (about 3 exchanges keeps prompts small). */
export const CHAT_HISTORY_TURNS = 3;

interface ChatState {
  sessionId: string | null;
  /** Oldest first. */
  turns: AnswerView[];
  /** Bumped by the Ask hotkey; the Chat input focuses when it changes. */
  focusTick: number;
  /** Bumped when something asks in Chat from elsewhere; the overlay switches to Chat. */
  showTick: number;
  requestShow: () => void;
  upsert: (v: AnswerView) => void;
  /** Loads saved chat for a session (PRD QA-8: history per session). */
  load: (sessionId: string | null) => Promise<void>;
  requestFocus: () => void;
  /** Screenshot taken for the next question (capture-and-ask, PRD SCR-4). */
  pending: Screenshot | null;
  capturing: boolean;
  captureError: string | null;
  /** Captures for `sessionId`, attaches it to the next question and focuses Chat. */
  capture: (sessionId: string, request: CaptureRequest) => Promise<Screenshot | null>;
  clearPending: () => void;
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
      type: s.type === "transcript" || s.type === "screenshot" ? s.type : "doc",
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
  showTick: 0,
  requestShow: () => set((s) => ({ showTick: s.showTick + 1 })),
  upsert: (v) =>
    set((s) => {
      const i = s.turns.findIndex((x) => x.id === v.id);
      if (i === -1) return { turns: [...s.turns, v] };
      const turns = s.turns.slice();
      turns[i] = v;
      return { turns };
    }),
  load: async (sessionId) => {
    set({ sessionId, turns: [], pending: null, captureError: null });
    if (!sessionId) return;
    const r = await commands.listAnswers(sessionId, null);
    if (r.status !== "ok" || get().sessionId !== sessionId) return;
    const mine = r.data.filter((a) => a.source !== "detected").map(fromSaved);
    // Keep anything asked while loading.
    set((s) => ({ turns: [...mine, ...s.turns] }));
  },
  requestFocus: () => set((s) => ({ focusTick: s.focusTick + 1 })),
  pending: null,
  capturing: false,
  captureError: null,
  capture: async (sessionId, request) => {
    set({ capturing: true, captureError: null });
    const r = await commands.captureScreenshot(sessionId, request);
    if (r.status === "error") {
      set({ capturing: false, captureError: r.error.message });
      return null;
    }
    set((s) => ({ capturing: false, pending: r.data, focusTick: s.focusTick + 1 }));
    return r.data;
  },
  clearPending: () => set({ pending: null }),
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
