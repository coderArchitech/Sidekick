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
    <section aria-labelledby={headingId} className="rounded-lg border border-slate-700 p-4">
      <h2 id={headingId} className="text-base font-semibold">
        Your data
      </h2>
      <p className="mb-3 text-sm text-slate-400">
        Everything stays on this computer, encrypted. Deleting removes transcripts, notes, answers,
        screenshots and documents for good. Settings, keys and Skills stay; the record that
        participants agreed to transcription is kept for a year.
      </p>
      {message && (
        <p
          role={message.error ? "alert" : "status"}
          className={`mb-2 text-sm ${message.error ? "text-red-300" : "text-emerald-300"}`}
        >
          {message.text}
        </p>
      )}
      {asking ? (
        <div className="space-y-2 text-sm">
          <label className="block text-slate-300">
            Type DELETE to confirm
            <input
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              className="mt-1 block rounded-md border border-slate-600 bg-slate-950 px-2 py-1"
            />
          </label>
          <div className="flex gap-2">
            <button
              type="button"
              disabled={confirm !== "DELETE" || busy}
              className="rounded-md bg-red-700 px-3 py-1 text-white disabled:opacity-40"
              onClick={() => void deleteAll()}
            >
              Delete all meeting data
            </button>
            <button
              type="button"
              className="rounded-md px-3 py-1 hover:bg-slate-800"
              onClick={() => setAsking(false)}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          className="rounded-md border border-red-800 px-3 py-1 text-sm text-red-300 hover:bg-red-950"
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
