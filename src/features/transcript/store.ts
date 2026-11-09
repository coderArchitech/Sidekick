import { create } from "zustand";
import {
  commands,
  type Bookmark,
  type Session,
  type TranscriptLine,
  type TranscriptPartial,
  type TranscriptSegment,
} from "@/ipc";

type Stream = TranscriptPartial["stream"];

interface TranscriptState {
  sessionId: string | null;
  lines: TranscriptLine[];
  /** Latest interim text per stream; cleared when that stream's final segment arrives. */
  partials: Partial<Record<Stream, string>>;
  bookmarks: Bookmark[];
  load: (sessionId: string, lines: TranscriptLine[], bookmarks: Bookmark[]) => void;
  /** Applies a batch of events in arrival order (see lib/batcher). */
  applyEvents: (events: TranscriptEvent[]) => void;
  renameLocal: (from: string, to: string) => void;
  addBookmarkLocal: (b: Bookmark) => void;
}

export type TranscriptEvent =
  { kind: "partial"; event: TranscriptPartial } | { kind: "segment"; event: TranscriptSegment };

const streamOf = (seg: TranscriptSegment): Stream => (seg.isUser ? "mic" : "system");

export const useTranscriptStore = create<TranscriptState>((set) => ({
  sessionId: null,
  lines: [],
  partials: {},
  bookmarks: [],

  load: (sessionId, lines, bookmarks) => set({ sessionId, lines, bookmarks, partials: {} }),

  applyEvents: (events) =>
    set((state) => {
      let { lines, partials } = state;
      for (const item of events) {
        if (item.event.sessionId !== state.sessionId) continue;
        if (item.kind === "partial") {
          partials = { ...partials, [item.event.stream]: item.event.text };
        } else {
          const e = item.event;
          const line: TranscriptLine = {
            id: e.id,
            speakerLabel: e.speakerLabel,
            isUser: e.isUser,
            text: e.text,
            startMs: e.startMs,
            endMs: e.endMs,
          };
          lines = [...lines, line].sort((a, b) => a.startMs - b.startMs || a.id - b.id);
          partials = { ...partials, [streamOf(item.event)]: undefined };
        }
      }
      return { lines, partials };
    }),

  renameLocal: (from, to) =>
    set((state) => ({
      lines: state.lines.map((l) => (l.speakerLabel === from ? { ...l, speakerLabel: to } : l)),
    })),

  addBookmarkLocal: (b) =>
    set((state) => ({ bookmarks: [...state.bookmarks, b].sort((x, y) => x.atMs - y.atMs) })),
}));

/**
 * "me" → "Me", "speaker_2" → "Speaker 2", "room" (microphone only, no system
 * audio) → "Room"; names the user gave are kept as they are.
 */
export function displayName(label: string): string {
  if (label === "me") return "Me";
  if (label === "room") return "Room";
  const m = /^speaker_(\d+)$/.exec(label);
  return m ? `Speaker ${m[1]}` : label;
}

/** 754000 → "12:34". */
export function clock(ms: number): string {
  const total = Math.floor(ms / 1000);
  const m = Math.floor(total / 60);
  const s = total % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Bookmarks "now" in a running session (button and shortcut). */
export async function bookmarkNow(session: Session | null): Promise<boolean> {
  if (!session?.startedAt || session.status === "ended") return false;
  const r = await commands.addBookmark(
    session.id,
    Math.max(0, Date.now() - session.startedAt),
    null,
  );
  if (r.status === "ok") useTranscriptStore.getState().addBookmarkLocal(r.data);
  return r.status === "ok";
}
