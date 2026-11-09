import { useEffect, useRef } from "react";
import { answerEvents, commands, type Session } from "@/ipc";
import { modelPlan, sessionPlan, type ModelPlan } from "./models";
import { NOTES_INTERVAL_MS, NotesJob } from "./notes";
import { useNotesStore } from "./notesStore";

/**
 * Runs the rolling-notes job for `session` (PRD MEM-1, tech doc 3.3): every
 * 45 s while live, only when there's new transcript. When a session this
 * overlay watched goes from live or paused to ended, writes the summary with
 * decisions and action items (MEM-3). Failures leave the notes as they were.
 */
export function useNotes(session: Session | null, intervalMs = NOTES_INTERVAL_MS) {
  const sessionId = session?.id ?? null;
  const status = session?.status ?? null;
  // The session's own answer provider (PRD KEY-9).
  const llmProvider = session?.llmProvider ?? null;
  const job = useRef<NotesJob | null>(null);
  const sawActive = useRef(false);

  useEffect(() => {
    useNotesStore.setState({ sessionId, notes: null, activity: "idle", error: null });
    job.current = null;
    sawActive.current = false;
    if (!sessionId) return;
    const own = llmProvider ? { llmProvider } : null;
    let models: ModelPlan = modelPlan({ provider: llmProvider ?? "anthropic", model: null });
    void commands.getSettings().then((r) => {
      if (r.status === "ok") models = sessionPlan(r.data.llm, own);
    });
    const current = (id: string) => useNotesStore.getState().sessionId === id;
    job.current = new NotesJob(
      { ...commands, events: answerEvents },
      sessionId,
      () => models,
      (notes) => {
        if (current(sessionId)) useNotesStore.setState({ notes, error: null });
      },
    );
    void commands.getNotes(sessionId).then((r) => {
      if (r.status === "ok" && current(sessionId) && !useNotesStore.getState().notes)
        useNotesStore.setState({ notes: r.data });
    });
    return () => {
      job.current = null;
    };
  }, [sessionId, llmProvider]);

  useEffect(() => {
    const j = job.current;
    if (!j) return;
    if (status === "live" || status === "paused") sawActive.current = true;
    if (status === "live") {
      const run = () => {
        useNotesStore.setState({ activity: "updating" });
        j.tick()
          .catch((e: unknown) =>
            useNotesStore.setState({ error: e instanceof Error ? e.message : String(e) }),
          )
          .finally(() => {
            if (useNotesStore.getState().activity === "updating")
              useNotesStore.setState({ activity: "idle" });
          });
      };
      const timer = setInterval(run, intervalMs);
      return () => clearInterval(timer);
    }
    if (status === "ended" && sawActive.current) {
      sawActive.current = false;
      useNotesStore.setState({ activity: "summarizing" });
      // Catch up on the last lines first, then recap.
      void j
        .catchUp()
        .catch(() => null)
        .then(() => j.summarize())
        .catch((e: unknown) =>
          useNotesStore.setState({ error: e instanceof Error ? e.message : String(e) }),
        )
        .finally(() => useNotesStore.setState({ activity: "idle" }));
    }
    return undefined;
  }, [status, sessionId, intervalMs]);
}
