import { useId, useState } from "react";
import { commands } from "@/ipc";

/** Settings → Your data: delete every meeting at once (tech doc 4.4; PRD 6). */
export function DataSection() {
  const headingId = useId();
  const [confirm, setConfirm] = useState("");
  const [asking, setAsking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<{ text: string; error: boolean } | null>(null);

  async function deleteAll() {
    setBusy(true);
    const r = await commands.deleteAllData();
    setBusy(false);
    setAsking(false);
    setConfirm("");
    setMessage(
      r.status === "ok"
        ? {
            text: `Deleted ${r.data} ${r.data === 1 ? "session" : "sessions"} and all their data.`,
            error: false,
          }
        : { text: r.error.message, error: true },
    );
  }

  return (
    <section aria-labelledby={headingId} className="card p-4">
      <h2 id={headingId} className="heading text-xl">
        Your data
      </h2>
      <p className="mb-3 text-sm text-fg-muted">
        Everything stays on this computer, encrypted. Deleting removes transcripts, notes, answers,
        screenshots and documents for good. Settings, keys and Skills stay; the record that
        participants agreed to transcription is kept for a year.
      </p>
      {message && (
        <p
          role={message.error ? "alert" : "status"}
          className={`mb-2 text-sm ${message.error ? "text-danger" : "text-success"}`}
        >
          {message.text}
        </p>
      )}
      {asking ? (
        <div className="space-y-2 text-sm">
          <label className="block text-fg-2">
            Type DELETE to confirm
            <input
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              className="field mt-1 block"
            />
          </label>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={confirm !== "DELETE" || busy}
              className="btn btn-danger"
              onClick={() => void deleteAll()}
            >
              Delete all meeting data
            </button>
            <button type="button" className="btn btn-ghost" onClick={() => setAsking(false)}>
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          className="btn btn-danger-ghost"
          onClick={() => {
            setMessage(null);
            setAsking(true);
          }}
        >
          Delete everything…
        </button>
      )}
    </section>
  );
}
