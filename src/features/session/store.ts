import { create } from "zustand";
import {
  commands,
  type ConsentOption,
  type Retention,
  type Session,
  type SidekickError,
} from "@/ipc";

interface SessionState {
  session: Session | null;
  warnings: string[];
  error: string | null;
  busy: boolean;
  create: (title: string) => Promise<void>;
  /** Records consent, then starts listening (PRD 8). */
  confirmAndStart: (option: ConsentOption) => Promise<void>;
  pause: () => Promise<void>;
  resume: () => Promise<void>;
  end: () => Promise<void>;
  /** Auto-answer on/off for this session (QA-3). */
  toggleAuto: () => Promise<void>;
  /** Sets the session's active Skill, or none (PRD SKL-1). */
  setSkill: (skillId: string | null) => Promise<void>;
  /** How long this session's data is kept (tech doc 4.4). "session" deletes an ended one now. */
  setRetention: (retention: Retention) => Promise<void>;
  /** Deletes the session and everything in it. */
  remove: () => Promise<void>;
  /** Clears an ended session so a new one can be created. */
  reset: () => void;
  /** Applies a `session-changed` event for the current session. */
  applyChange: (session: Session) => void;
}

const message = (e: SidekickError) => e.message;

export const useSessionStore = create<SessionState>((set, get) => {
  async function run<T>(
    op: () => Promise<{ status: "ok"; data: T } | { status: "error"; error: SidekickError }>,
  ) {
    set({ busy: true, error: null });
    const result = await op();
    set({ busy: false });
    if (result.status === "error") {
      set({ error: message(result.error) });
      return null;
    }
    return result.data;
  }

  return {
    session: null,
    warnings: [],
    error: null,
    busy: false,

    create: async (title) => {
      const session = await run(() => commands.createSession(title));
      if (session) set({ session, warnings: [] });
    },

    confirmAndStart: async (option) => {
      const current = get().session;
      if (!current) return;
      const consented = await run(() => commands.confirmConsent(current.id, option));
      if (!consented) return;
      set({ session: consented });
      const started = await run(() => commands.startSession(current.id));
      if (started) set({ session: started.session, warnings: started.warnings });
    },

    pause: async () => {
      const id = get().session?.id;
      if (!id) return;
      const s = await run(() => commands.pauseSession(id));
      if (s) set({ session: s });
    },

    resume: async () => {
      const id = get().session?.id;
      if (!id) return;
      const s = await run(() => commands.resumeSession(id));
      if (s) set({ session: s });
    },

    end: async () => {
      const id = get().session?.id;
      if (!id) return;
      const s = await run(() => commands.endSession(id));
      if (s) set({ session: s, warnings: [] });
    },

    setSkill: async (skillId) => {
      const s = get().session;
      if (!s || s.status === "ended") return;
      const updated = await run(() => commands.setSessionSkill(s.id, skillId));
      if (updated) set({ session: updated });
    },
    toggleAuto: async () => {
      const s = get().session;
      if (!s || s.status === "ended") return;
      const updated = await run(() => commands.setAutoAnswer(s.id, !s.autoAnswer));
      if (updated) set({ session: updated });
    },

    setRetention: async (retention) => {
      const s = get().session;
      if (!s) return;
      const updated = await run(() => commands.setSessionRetention(s.id, retention));
      if (!updated) return;
      if (updated.status === "ended" && retention === "session") get().reset();
      else set({ session: updated });
    },
    remove: async () => {
      const s = get().session;
      if (!s) return;
      set({ busy: true, error: null });
      const r = await commands.deleteSession(s.id);
      set({ busy: false });
      if (r.status === "error") set({ error: message(r.error) });
      else get().reset();
    },
    reset: () => set({ session: null, warnings: [], error: null }),

    applyChange: (session) => {
      if (get().session?.id === session.id) set({ session });
    },
  };
});
