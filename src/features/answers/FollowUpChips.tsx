import { useState } from "react";
import { askFollowUp, canFollowUp, FOLLOW_UPS, type AnswerView } from "@/orchestrator";

/** "Shorter", "More detail", "Give an example", "Explain simply" under a finished answer (PRD QA-9). */
export function FollowUpChips({ view }: { view: AnswerView }) {
  const [error, setError] = useState<string | null>(null);
  if (!canFollowUp(view)) return null;
  return (
    <div className="mt-2">
      <div role="group" aria-label="Follow-ups" className="flex flex-wrap gap-1">
        {FOLLOW_UPS.map((f) => (
          <button
            key={f.kind}
            type="button"
            className="rounded-full border border-line-strong px-2.5 py-0.5 text-xs text-fg-2 transition-colors hover:border-accent hover:text-fg"
            onClick={() => void askFollowUp(view, f.kind).then(setError)}
          >
            {f.label}
          </button>
        ))}
      </div>
      {error && (
        <p role="alert" className="mt-1 text-xs text-danger">
          {error}
        </p>
      )}
    </div>
  );
}
