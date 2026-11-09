import { Markdown } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import { useEffect } from "react";
import { commands } from "@/ipc";
import { useNotesStore } from "@/orchestrator";
import { Agenda } from "./Agenda";
import { MissedRecap } from "./MissedRecap";

/**
 * The Notes tab (PRD MEM-1 to MEM-4): "What did I miss?", the agenda, rolling
 * notes, then the summary with action items once the session ends.
 */
export function NotesView() {
  const session = useSessionStore((s) => s.session);
  const { notes, activity, error } = useNotesStore();

  const sessionId = session?.id;
  // Opening the tab shows what's saved, including changes made elsewhere.
  useEffect(() => {
    if (!sessionId) return;
    void commands.getNotes(sessionId).then((r) => {
      if (r.status === "ok" && useNotesStore.getState().sessionId === sessionId)
        useNotesStore.setState({ notes: r.data });
    });
  }, [sessionId]);

  if (!session) return <p className="text-fg-muted">Start a session to keep notes.</p>;

  const empty = !notes?.contentMd.trim() && !notes?.summaryMd && !notes?.actionItems.length;
  const started = session.status !== "draft";
  return (
    <div className="space-y-3">
      {started && <MissedRecap sessionId={session.id} />}
      <Agenda sessionId={session.id} notes={notes} />
      {activity !== "idle" && (
        <p role="status" className="text-xs text-fg-muted">
          {activity === "summarizing" ? "Writing the summary…" : "Updating notes…"}
        </p>
      )}
      {error && (
        <p role="alert" className="text-xs text-warning">
          Couldn't update the notes: {error}
        </p>
      )}

      {notes?.summaryMd && (
        <section aria-label="Summary" className="rounded-lg border border-line p-2">
          <h2 className="text-sm font-semibold text-fg">Summary</h2>
          <div className="text-sm text-fg-2">
            <Markdown text={notes.summaryMd} />
          </div>
          {notes.actionItems.length > 0 && (
            <>
              <h3 className="mt-2 text-sm font-semibold text-fg">Action items</h3>
              <ul aria-label="Action items" className="space-y-1 text-sm text-fg-2">
                {notes.actionItems.map((a) => (
                  <li key={a.id}>
                    <label className="flex items-start gap-2">
                      <input
                        type="checkbox"
                        className="mt-1"
                        checked={a.done}
                        onChange={(e) =>
                          void commands
                            .setActionItemDone(session.id, a.id, e.target.checked)
                            .then((r) => {
                              if (r.status === "ok") useNotesStore.setState({ notes: r.data });
                            })
                        }
                      />
                      <span className={a.done ? "text-fg-muted line-through" : undefined}>
                        {a.text}
                        {a.owner && <span className="text-fg-muted"> — {a.owner}</span>}
                        {a.due && <span className="text-fg-muted"> (due {a.due})</span>}
                      </span>
                    </label>
                  </li>
                ))}
              </ul>
            </>
          )}
        </section>
      )}

      {notes?.contentMd.trim() ? (
        <section
          aria-label="Notes"
          className="text-sm text-fg-2 [&_h2]:mt-2 [&_h2]:font-semibold [&_h2]:text-fg"
        >
          <Markdown text={notes.contentMd} />
        </section>
      ) : (
        empty &&
        activity === "idle" && (
          <p className="text-fg-muted">
            {session.status === "ended"
              ? "No notes for this session."
              : "Notes appear here as the meeting goes on (every 45 seconds)."}
          </p>
        )
      )}
    </div>
  );
}
