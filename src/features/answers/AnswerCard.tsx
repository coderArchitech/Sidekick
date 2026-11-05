import { useState } from "react";
import { commands } from "@/ipc";
import { activeRouter, type AnswerView } from "@/orchestrator";
import { Markdown } from "./Markdown";

/** One answer: cue first, expandable full answer, then sources (PRD QA-5, QA-6). */
export function AnswerCard({ view }: { view: AnswerView }) {
  const [open, setOpen] = useState(view.directed);
  const [rating, setRating] = useState<number | null>(null);
  const streaming = view.status === "thinking" || view.status === "streaming";

  async function rate(value: 1 | -1) {
    if (!view.answerId) return;
    const next = rating === value ? 0 : value;
    const r = await commands.rateAnswer(view.answerId, next);
    if (r.status === "ok") setRating(next === 0 ? null : next);
  }

  return (
    <article
      aria-label={`Answer to: ${view.question}`}
      className={`rounded-lg border p-2 ${view.directed ? "border-sky-600" : "border-slate-700"}`}
    >
      <header className="flex items-start gap-2 text-xs text-slate-400">
        <span className="min-w-0 flex-1">
          {view.directed && (
            <span className="mr-1 rounded bg-sky-900 px-1 text-sky-200">For you</span>
          )}
          {view.question}
        </span>
        {streaming && (
          <button
            type="button"
            className="rounded px-1 hover:bg-slate-800"
            onClick={() => activeRouter()?.router.cancel(view.id)}
          >
            Stop
          </button>
        )}
      </header>

      <p className="mt-1 text-base font-medium text-slate-50" aria-live="polite">
        {view.cue ||
          (view.status === "thinking" ? "Thinking…" : view.status === "streaming" ? "…" : "")}
      </p>

      {view.status === "error" && (
        <p role="alert" className="mt-1 text-sm text-red-300">
          {view.error}
        </p>
      )}
      {view.status === "cancelled" && <p className="mt-1 text-xs text-slate-500">Stopped.</p>}

      {(view.body || view.sources.length > 0) && view.body !== view.cue && (
        <>
          {view.cue && (
            <button
              type="button"
              aria-expanded={open}
              className="mt-1 text-xs text-slate-400 hover:text-slate-200"
              onClick={() => setOpen((o) => !o)}
            >
              {open ? "▾ Hide full answer" : "▸ Full answer"}
            </button>
          )}
          {/* No cue (a full-only Skill): the answer itself is the content. */}
          {(open || !view.cue) && (
            <div className="mt-1 text-sm text-slate-200">
              <Markdown text={view.body} />
              {view.sources.length > 0 && (
                <ol aria-label="Sources" className="mt-1 space-y-0.5 text-xs text-slate-400">
                  {view.sources.map((s) => (
                    <li key={s.n}>
                      [{s.n}] {s.label}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          )}
        </>
      )}

      {view.status === "done" && view.answerId && (
        <div className="mt-1 flex gap-1 text-xs">
          <button
            type="button"
            aria-label="Helpful"
            aria-pressed={rating === 1}
            className={`rounded px-1.5 hover:bg-slate-800 ${rating === 1 ? "text-emerald-300" : "text-slate-500"}`}
            onClick={() => void rate(1)}
          >
            👍
          </button>
          <button
            type="button"
            aria-label="Not helpful"
            aria-pressed={rating === -1}
            className={`rounded px-1.5 hover:bg-slate-800 ${rating === -1 ? "text-red-300" : "text-slate-500"}`}
            onClick={() => void rate(-1)}
          >
            👎
          </button>
        </div>
      )}
    </article>
  );
}
