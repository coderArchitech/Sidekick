import { useEffect, useId, useState } from "react";
import { sessionEvents } from "@/ipc";
import { ConsentDialog } from "./ConsentDialog";
import { useSessionStore } from "./store";

const button =
  "rounded-md border border-slate-600 px-2 py-0.5 text-xs hover:bg-slate-700 disabled:opacity-50";

/** Session controls in the overlay: create, start (with consent), pause, resume, end (PRD CTX-1, LIS-6). */
export function SessionBar() {
  const {
    session,
    warnings,
    error,
    busy,
    create,
    confirmAndStart,
    pause,
    resume,
    end,
    reset,
    applyChange,
  } = useSessionStore();
  const [title, setTitle] = useState("");
  const [asking, setAsking] = useState(false);
  const inputId = useId();

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void sessionEvents.onSessionChanged((e) => applyChange(e.session)).then((u) => (unlisten = u));
    return () => unlisten?.();
  }, [applyChange]);

  return (
    <div className="space-y-1 border-b border-slate-700 px-3 py-2 text-xs">
      {!session || session.status === "ended" ? (
        <form
          className="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (session) reset();
            void create(title).then(() => setTitle(""));
          }}
        >
          {session?.status === "ended" && <span className="text-slate-400">Session ended.</span>}
          <label htmlFor={inputId} className="sr-only">
            Session name
          </label>
          <input
            id={inputId}
            value={title}
            placeholder="New session name"
            onChange={(e) => setTitle(e.target.value)}
            className="min-w-0 flex-1 rounded-md border border-slate-600 bg-slate-950 px-2 py-0.5"
          />
          <button type="submit" className={button} disabled={busy || !title.trim()}>
            New session
          </button>
        </form>
      ) : (
        <div className="flex items-center gap-2">
          <span className="min-w-0 flex-1 truncate font-medium" title={session.title}>
            {session.title}
          </span>
          {session.status === "draft" && (
            <button
              type="button"
              className={button}
              disabled={busy}
              onClick={() => setAsking(true)}
            >
              Start
            </button>
          )}
          {session.status === "live" && (
            <>
              <span role="status" className="flex items-center gap-1 text-red-300">
                <span
                  aria-hidden="true"
                  className="inline-block h-2 w-2 animate-pulse rounded-full bg-red-500"
                />
                {session.consent === "me_only" ? "Listening (you only)" : "Listening"}
              </span>
              <button type="button" className={button} disabled={busy} onClick={() => void pause()}>
                Pause
              </button>
            </>
          )}
          {session.status === "paused" && (
            <>
              <span role="status" className="text-amber-300">
                Paused
              </span>
              <button
                type="button"
                className={button}
                disabled={busy}
                onClick={() => void resume()}
              >
                Resume
              </button>
            </>
          )}
          <button type="button" className={button} disabled={busy} onClick={() => void end()}>
            End
          </button>
        </div>
      )}
      {warnings.map((w) => (
        <p key={w} role="status" className="text-amber-300">
          {w}
        </p>
      ))}
      {error && (
        <p role="alert" className="text-red-300">
          {error}
        </p>
      )}
      {asking && (
        <ConsentDialog
          onCancel={() => setAsking(false)}
          onConfirm={(option) => {
            setAsking(false);
            void confirmAndStart(option);
          }}
        />
      )}
    </div>
  );
}
