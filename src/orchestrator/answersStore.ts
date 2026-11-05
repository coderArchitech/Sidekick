import { create } from "zustand";
import type { AnswerView } from "./answerer";

interface AnswersState {
  /** Newest first, at most 30. */
  views: AnswerView[];
  upsert: (v: AnswerView) => void;
  clear: () => void;
}

export const useAnswersStore = create<AnswersState>((set) => ({
  views: [],
  upsert: (v) =>
    set((s) => {
      const i = s.views.findIndex((x) => x.id === v.id);
      if (i === -1) return { views: [v, ...s.views].slice(0, 30) };
      const views = s.views.slice();
      views[i] = v;
      return { views };
    }),
  clear: () => set({ views: [] }),
}));
