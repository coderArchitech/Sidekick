import { create } from "zustand";
import type { DetectedQuestion } from "./detector";

export interface QueuedQuestion extends DetectedQuestion {
  /** Answer it now (directed, or Auto-answer on), per QA-3. */
  answer: boolean;
}

interface OrchestratorState {
  /** Most recent first, at most 50. */
  questions: QueuedQuestion[];
  add: (q: QueuedQuestion) => void;
  clear: () => void;
}

export const useOrchestratorStore = create<OrchestratorState>((set) => ({
  questions: [],
  add: (q) => set((s) => ({ questions: [q, ...s.questions].slice(0, 50) })),
  clear: () => set({ questions: [] }),
}));
