import { useEffect, useState } from "react";
import { commands, type PackInfo } from "@/ipc";

/**
 * Turns Knowledge Packs on or off for a session (PRD CTX-5), in the
 * overlay's Documents panel. Answers then search the pack's documents too.
 */
export function PackPicker({ sessionId }: { sessionId: string }) {
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [on, setOn] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void Promise.all([commands.listPacks(), commands.listSessionPacks(sessionId)]).then(
      ([p, s]) => {
        if (!live) return;
        if (p.status === "ok") setPacks(p.data);
        if (s.status === "ok") setOn(s.data);
      },
    );
    return () => {
      live = false;
    };
  }, [sessionId]);

  async function toggle(id: string, checked: boolean) {
    const next = checked ? [...on, id] : on.filter((x) => x !== id);
    const r = await commands.setSessionPacks(sessionId, next);
    if (r.status === "error") setError(r.error.message);
    else {
      setError(null);
      setOn(r.data);
    }
  }

  return (
    <fieldset className="space-y-1">
      <legend className="text-fg-2">Knowledge Packs</legend>
      {packs.length === 0 ? (
        <p className="text-fg-muted">Create packs in Settings → Knowledge Packs.</p>
      ) : (
        packs.map((p) => (
          <label key={p.id} className="flex items-center gap-2">
            <input
              type="checkbox"
              aria-label={`Use ${p.name} in this session`}
              checked={on.includes(p.id)}
              onChange={(e) => void toggle(p.id, e.target.checked)}
            />
            <span className="min-w-0 flex-1 truncate">{p.name}</span>
            <span className="text-fg-muted tabular">
              {p.ready}/{p.documents} ready
            </span>
          </label>
        ))
      )}
      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
    </fieldset>
  );
}
