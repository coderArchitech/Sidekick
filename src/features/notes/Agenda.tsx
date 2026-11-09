import { useState } from "react";
import { commands, type AgendaStatus, type Notes } from "@/ipc";
import { Select } from "@/lib/Select";
import { useNotesStore } from "@/orchestrator";

const LABEL: Record<AgendaStatus, string> = {
  not_started: "Not yet",
  in_progress: "In progress",
  covered: "Covered",
};

const DOT: Record<AgendaStatus, string> = {
  not_started: "border border-line-strong",
  in_progress: "bg-warning",
  covered: "bg-success",
};

/**
 * The session's agenda (PRD MEM-2): one item per line, each marked covered,
 * in progress or not yet discussed. The notes job moves statuses on as the
 * meeting goes; the user can always set one by hand.
 */
export function Agenda({ sessionId, notes }: { sessionId: string; notes: Notes | null }) {
  const agenda = notes?.agenda ?? [];
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [error, setError] = useState<string | null>(null);

  const apply = (r: Awaited<ReturnType<typeof commands.setAgenda>>) => {
    if (r.status === "error") {
      setError(r.error.message);
      return false;
    }
    setError(null);
    useNotesStore.setState({ notes: r.data });
    return true;
  };

  const save = async () => {
    if (apply(await commands.setAgenda(sessionId, draft.split("\n")))) setEditing(false);
  };

  const covered = agenda.filter((a) => a.status === "covered").length;

  return (
    <section aria-label="Agenda" className="card space-y-2 p-2.5">
      <div className="flex items-center gap-2">
        <h2 className="text-sm font-semibold text-fg">Agenda</h2>
        {agenda.length > 0 && (
          <span className="text-xs text-fg-muted tabular">
            {covered}/{agenda.length} covered
          </span>
        )}
        {!editing && (
          <button
            type="button"
            className="btn btn-ghost btn-sm ml-auto"
            onClick={() => {
              setDraft(agenda.map((a) => a.text).join("\n"));
              setEditing(true);
            }}
          >
            {agenda.length ? "Edit agenda" : "Add agenda"}
          </button>
        )}
      </div>

      {editing ? (
        <div className="space-y-2">
          <textarea
            aria-label="Agenda items, one per line"
            className="field w-full"
            rows={4}
            placeholder={"Pricing\nRoadmap\nHiring"}
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
          />
          <div className="flex justify-end gap-1">
            <button
              type="button"
              className="btn btn-ghost btn-sm"
              onClick={() => setEditing(false)}
            >
              Cancel
            </button>
            <button type="button" className="btn btn-primary btn-sm" onClick={() => void save()}>
              Save agenda
            </button>
          </div>
        </div>
      ) : (
        agenda.length > 0 && (
          <ol aria-label="Agenda items" className="space-y-1">
            {agenda.map((a) => (
              <li key={a.id} className="flex items-center gap-2 text-sm">
                <span
                  aria-hidden="true"
                  className={`h-2 w-2 shrink-0 rounded-full ${DOT[a.status]}`}
                />
                <span
                  className={`min-w-0 flex-1 ${a.status === "covered" ? "text-fg-muted" : "text-fg-2"}`}
                >
                  {a.text}
                </span>
                <Select
                  aria-label={`Status of ${a.text}`}
                  small
                  value={a.status}
                  onChange={(v) =>
                    void commands
                      .updateAgenda(sessionId, [{ id: a.id, status: v as AgendaStatus }])
                      .then(apply)
                  }
                  options={(Object.keys(LABEL) as AgendaStatus[]).map((s) => ({
                    value: s,
                    label: LABEL[s],
                  }))}
                />
              </li>
            ))}
          </ol>
        )
      )}
      {error && (
        <p role="alert" className="text-xs text-warning">
          {error}
        </p>
      )}
    </section>
  );
}
