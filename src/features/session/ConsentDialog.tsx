import { useEffect, useId, useRef, useState } from "react";
import type { ConsentOption } from "@/ipc";

interface Props {
  onConfirm: (option: ConsentOption) => void;
  onCancel: () => void;
}

const OPTIONS: { value: ConsentOption; label: string; detail: string }[] = [
  {
    value: "participants_agreed",
    label: "Participants know and agree to transcription",
    detail: "Sidekick transcribes your microphone and the meeting audio.",
  },
  {
    value: "me_only",
    label: "Transcribe me only",
    detail: "Sidekick transcribes only your microphone. Other people are never recorded.",
  },
];

/**
 * Asked before every session starts (PRD 8, "Session start confirmation").
 * Listening to others can't begin until the user picks an option here.
 */
export function ConsentDialog({ onConfirm, onCancel }: Props) {
  const titleId = useId();
  const [choice, setChoice] = useState<ConsentOption | null>(null);
  const first = useRef<HTMLInputElement>(null);

  useEffect(() => {
    first.current?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onCancel();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [onCancel]);

  return (
    <div className="fixed inset-0 z-10 flex items-center justify-center bg-scrim p-3">
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        className="w-full max-w-sm rounded-lg border border-line bg-popover p-4 text-sm"
      >
        <h2 id={titleId} className="heading text-xl">
          Before Sidekick listens
        </h2>
        <p className="mt-1 text-fg-muted">
          Recording rules differ by place and company. Sidekick never tells the meeting for you.
        </p>
        <fieldset className="mt-3 space-y-2">
          <legend className="sr-only">Who may be transcribed</legend>
          {OPTIONS.map((o, i) => (
            <div key={o.value} className="flex gap-2 rounded-md border border-line p-2">
              <input
                ref={i === 0 ? first : undefined}
                id={`${titleId}-${o.value}`}
                type="radio"
                name="consent"
                value={o.value}
                checked={choice === o.value}
                aria-describedby={`${titleId}-${o.value}-detail`}
                className="mt-1 h-4 w-4 shrink-0 accent-blue-500"
                onChange={() => setChoice(o.value)}
              />
              <div>
                <label htmlFor={`${titleId}-${o.value}`} className="block font-medium">
                  {o.label}
                </label>
                <p id={`${titleId}-${o.value}-detail`} className="text-fg-muted">
                  {o.detail}
                </p>
              </div>
            </div>
          ))}
        </fieldset>
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" className="btn btn-secondary" onClick={onCancel}>
            Cancel
          </button>
          <button
            type="button"
            className="btn btn-primary"
            disabled={!choice}
            onClick={() => choice && onConfirm(choice)}
          >
            Start listening
          </button>
        </div>
      </div>
    </div>
  );
}
