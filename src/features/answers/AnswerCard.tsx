import { useState } from "react";
import { commands } from "@/ipc";
import { activeRouter, type AnswerView } from "@/orchestrator";
import { FollowUpChips } from "./FollowUpChips";
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
      className={`card p-3 ${view.directed ? "border-accent/60 shadow-[0_0_0_1px_var(--color-accent-soft)]" : ""}`}
    >
      <header className="flex items-start gap-2 text-xs text-fg-muted">
        <span className="min-w-0 flex-1">
          {view.directed && <span className="badge badge-accent mr-1.5 align-[1px]">For you</span>}
          {view.question}
        </span>
        {streaming && (
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={() => activeRouter()?.router.cancel(view.id)}
          >
            Stop
          </button>
        )}
      </header>

      <p className="mt-1.5 text-[0.9375rem] leading-snug font-semibold text-fg" aria-live="polite">
        {view.cue ||
          (view.status === "thinking" ? "Thinking…" : view.status === "streaming" ? "…" : "")}
      </p>

      {view.status === "error" && (
        <p role="alert" className="mt-1 text-sm text-danger">
          {view.error}
        </p>
      )}
      {view.status === "cancelled" && <p className="mt-1 text-xs text-fg-muted">Stopped.</p>}

      {(view.body || view.sources.length > 0) && view.body !== view.cue && (
        <>
          {view.cue && (
            <button
              type="button"
              aria-expanded={open}
              className="mt-2 text-xs font-medium text-accent-text hover:underline"
              onClick={() => setOpen((o) => !o)}
            >
              {open ? "▾ Hide full answer" : "▸ Full answer"}
            </button>
          )}
          {/* No cue (a full-only Skill): the answer itself is the content. */}
          {(open || !view.cue) && (
            <div className="mt-2 border-t border-line pt-2 text-sm leading-relaxed text-fg-2">
              <Markdown text={view.body} />
              {view.sources.length > 0 && (
                <ol aria-label="Sources" className="mt-2 flex flex-wrap gap-1">
                  {view.sources.map((s) => (
                    <li key={s.n} className="badge bg-surface-3 font-normal text-fg-2">
                      [{s.n}] {s.label}
                    </li>
                  ))}
                </ol>
              )}
            </div>
          )}
        </>
      )}

      <FollowUpChips view={view} />

      {view.status === "done" && view.answerId && (
        <div className="mt-2 flex gap-0.5 text-xs">
          <button
            type="button"
            aria-label="Helpful"
            aria-pressed={rating === 1}
            className={`btn btn-ghost btn-sm px-1.5 ${rating === 1 ? "bg-success-soft" : "opacity-70"}`}
            onClick={() => void rate(1)}
          >
            👍
          </button>
          <button
            type="button"
            aria-label="Not helpful"
            aria-pressed={rating === -1}
            className={`btn btn-ghost btn-sm px-1.5 ${rating === -1 ? "bg-danger-soft" : "opacity-70"}`}
            onClick={() => void rate(-1)}
          >
            👎
          </button>
        </div>
      )}
    </article>
  );
}
