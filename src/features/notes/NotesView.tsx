import { Markdown } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import { useNotesStore } from "@/orchestrator";

/** The Notes tab (PRD MEM-1, MEM-3): rolling notes, then the summary once the session ends. */
export function NotesView() {
  const session = useSessionStore((s) => s.session);
  const { notes, activity, error } = useNotesStore();

  if (!session) return <p className="text-fg-muted">Start a session to keep notes.</p>;

  const empty = !notes?.contentMd.trim() && !notes?.summaryMd && !notes?.actionItems.length;
  return (
    <div className="space-y-3">
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
              <ul aria-label="Action items" className="ml-4 list-disc text-sm text-fg-2">
                {notes.actionItems.map((a) => (
                  <li key={a.id}>
                    {a.text}
                    {a.owner && <span className="text-fg-muted"> — {a.owner}</span>}
                    {a.due && <span className="text-fg-muted"> (due {a.due})</span>}
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
