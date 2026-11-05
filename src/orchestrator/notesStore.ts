import { create } from "zustand";
import type { Notes } from "@/ipc";

export type NotesActivity = "idle" | "updating" | "summarizing";

interface NotesState {
  sessionId: string | null;
  notes: Notes | null;
  activity: NotesActivity;
  /** Last job failure; notes keep their previous content (NFR-5). */
  error: string | null;
}

export const useNotesStore = create<NotesState>(() => ({
  sessionId: null,
  notes: null,
  activity: "idle",
  error: null,
}));
