import { useEffect, useId, useRef, useState } from "react";
import { useSessionStore } from "@/features/session";
import { commands } from "@/ipc";
import { useNotesStore } from "@/orchestrator";

/**
 * Waits for the end-of-session summary the overlay writes after End (T-024):
 * until notes go from "summarizing" back to idle. Gives up after `timeoutMs`,
 * and returns at once if no summary starts within `startMs`.
 */
export function waitForSummary(timeoutMs = 60_000, startMs = 2_000): Promise<void> {
  return new Promise((resolve) => {
    let started = useNotesStore.getState().activity === "summarizing";
    const timers: ReturnType<typeof setTimeout>[] = [];
    const finish = () => {
      unsubscribe();
      timers.forEach(clearTimeout);
      resolve();
    };
    const unsubscribe = useNotesStore.subscribe((s) => {
      if (s.activity === "summarizing") started = true;
      else if (started && s.activity === "idle") finish();
    });
    timers.push(setTimeout(() => !started && finish(), startMs));
    timers.push(setTimeout(finish, timeoutMs));
  });
}

/**
 * The red close button in the overlay header: quits Sidekick. A session that
 * is still listening is ended first, after asking, so its notes and summary
 * are saved.
 */
export function QuitButton() {
  const session = useSessionStore((s) => s.session);
  const [asking, setAsking] = useState(false);
  const [ending, setEnding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const titleId = useId();
  const cancelRef = useRef<HTMLButtonElement>(null);
  const active = session?.status === "live" || session?.status === "paused";

  useEffect(() => {
    if (!asking) return;
    cancelRef.current?.focus();
    // Escape cancels, unless the session is already ending.
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !ending) setAsking(false);
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, [asking, ending]);

  async function endThenQuit() {
    setError(null);
    setEnding(true);
    const summary = waitForSummary();
    await useSessionStore.getState().end();
    const { session: after, error: failed } = useSessionStore.getState();
    if (after?.status !== "ended") {
      setEnding(false);
      setError(failed ?? "Couldn't end the session.");
      return;
    }
    await summary;
    await commands.quitApp();
  }

  return (
    <>
      <button
        type="button"
        aria-label="Quit Sidekick"
        title="Quit Sidekick"
        className="btn btn-danger btn-sm h-6 w-6 rounded-full p-0 text-sm leading-none"
        onClick={() => (active ? setAsking(true) : void commands.quitApp())}
      >
        <span aria-hidden="true">✕</span>
      </button>
      {asking && (
        <div className="fixed inset-0 z-30 flex items-center justify-center bg-black/60 p-3">
          <div
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            className="w-full max-w-sm rounded-lg border border-line bg-popover p-4 text-sm"
          >
            <h2 id={titleId} className="text-base font-semibold">
              Quit Sidekick?
            </h2>
            {ending ? (
              <p role="status" className="mt-1 text-fg-2">
                Ending the session and writing its summary…
              </p>
            ) : (
              <p className="mt-1 text-fg-2">
                &ldquo;{session?.title}&rdquo; is still{" "}
                {session?.status === "paused" ? "paused" : "listening"}. Sidekick ends it and saves
                its notes and summary, then quits.
              </p>
            )}
            {error && (
              <p role="alert" className="mt-2 text-danger">
                {error}
              </p>
            )}
            <div className="mt-4 flex justify-end gap-2">
              {ending ? (
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void commands.quitApp()}
                >
                  Quit now
                </button>
              ) : (
                <>
                  <button
                    ref={cancelRef}
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => setAsking(false)}
                  >
                    Cancel
                  </button>
                  <button
                    type="button"
                    className="btn btn-danger"
                    onClick={() => void endThenQuit()}
                  >
                    End session and quit
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
