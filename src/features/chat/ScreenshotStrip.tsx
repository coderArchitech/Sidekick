import { useEffect, useState } from "react";
import { commands, type Screenshot } from "@/ipc";
import { askScreenshotAction, SCREENSHOT_ACTIONS, useChatStore } from "@/orchestrator";

// Thumbnails don't change: keep them for the life of the overlay.
const thumbs = new Map<string, string>();

/** A screenshot's image as a `data:` URL, loaded once. */
export function useThumb(id: string | null): string | null {
  const [, loaded] = useState(0);
  const cached = id ? (thumbs.get(id) ?? null) : null;
  useEffect(() => {
    if (!id || thumbs.has(id)) return;
    let live = true;
    void commands.screenshotImage(id).then((r) => {
      if (r.status !== "ok") return;
      thumbs.set(id, r.data);
      if (live) loaded((n) => n + 1);
    });
    return () => {
      live = false;
    };
  }, [id]);
  return cached;
}

const timeOfDay = (ms: number) =>
  new Date(ms).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });

/** Quick actions on the attached screenshot (PRD SCR-5). */
export function QuickActions({ disabled }: { disabled: boolean }) {
  const [error, setError] = useState<string | null>(null);
  return (
    <div>
      <div role="group" aria-label="Quick actions" className="flex flex-wrap gap-1">
        {SCREENSHOT_ACTIONS.map((a) => (
          <button
            key={a.kind}
            type="button"
            disabled={disabled}
            className="rounded-full border border-line-strong px-2.5 py-0.5 text-xs text-fg-2 transition-colors hover:border-accent hover:text-fg disabled:opacity-50"
            onClick={() => void askScreenshotAction(a.kind).then(setError)}
          >
            {a.label}
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

/** Thumbnails of every capture in the session, with re-ask and delete (PRD SCR-6). */
export function ScreenshotStrip({ disabled }: { disabled: boolean }) {
  const shots = useChatStore((s) => s.screenshots);
  const pending = useChatStore((s) => s.pending);
  if (shots.length === 0) return null;
  return (
    <section aria-label="Screenshots" className="space-y-1">
      <h2 className="text-xs text-fg-muted">Screenshots ({shots.length})</h2>
      <ul className="flex gap-1.5 overflow-x-auto pb-1">
        {shots.map((s) => (
          <Thumb key={s.id} shot={s} attached={pending?.id === s.id} disabled={disabled} />
        ))}
      </ul>
    </section>
  );
}

function Thumb({
  shot,
  attached,
  disabled,
}: {
  shot: Screenshot;
  attached: boolean;
  disabled: boolean;
}) {
  const src = useThumb(shot.id);
  const attach = useChatStore((s) => s.attach);
  const remove = useChatStore((s) => s.removeScreenshot);
  const when = timeOfDay(shot.createdAt);
  return (
    <li className="relative shrink-0">
      <button
        type="button"
        aria-label={`Ask about the screenshot from ${when}`}
        aria-pressed={attached}
        disabled={disabled}
        className={`block h-12 w-20 overflow-hidden rounded-md border ${
          attached ? "border-accent ring-2 ring-accent/40" : "border-line hover:border-line-strong"
        }`}
        onClick={() => attach(shot)}
      >
        {src ? (
          <img src={src} alt="" className="h-full w-full object-cover" />
        ) : (
          <span className="text-[10px] text-fg-muted">{when}</span>
        )}
      </button>
      <button
        type="button"
        aria-label={`Delete the screenshot from ${when}`}
        className="absolute -top-1 -right-1 grid h-4 w-4 place-items-center rounded-full bg-popover text-[10px] text-fg-2 shadow hover:text-danger"
        onClick={() => void remove(shot.id)}
      >
        ✕
      </button>
    </li>
  );
}
