import { useId, useState } from "react";
import { chooseLogExportPath, commands, type ProblemKind, type StartupProblem } from "@/ipc";

type Action = "startFresh" | "restoreBackup" | "retry";

interface Copy {
  title: string;
  body: (p: StartupProblem) => string;
  next: (p: StartupProblem) => string | null;
  actions: (p: StartupProblem) => Action[];
}

const COPY: Record<ProblemKind, Copy> = {
  key_missing: {
    title: "Sidekick can't unlock your saved data",
    body: (p) =>
      `The key that unlocks Sidekick's data is missing from ${p.keychainName}. It may have been deleted, or ${p.keychainName} was reset. Without it, your saved sessions can't be read.`,
    next: (p) =>
      `If you can restore the "sidekick" entry from a ${p.keychainName} backup, do that and choose Try again. Otherwise, start fresh.`,
    actions: () => ["startFresh", "retry"],
  },
  keychain_unavailable: {
    title: "Sidekick couldn't read its key",
    body: (p) =>
      `${p.keychainName} is locked, or Sidekick wasn't allowed to read its key. Your data is safe.`,
    next: (p) =>
      `Unlock ${p.keychainName}, then choose Try again. If you're asked for a password, use the one you log in with and choose Always Allow.`,
    actions: () => ["retry"],
  },
  database_unreadable: {
    title: "Sidekick can't open its data file",
    body: () =>
      "The data file may be damaged, or an upgrade didn't finish. Your key is still there.",
    next: (p) =>
      p.hasBackup
        ? "Restore backup puts back the copy made before the last upgrade. If that doesn't help, start fresh."
        : "Try again, or start fresh.",
    actions: (p) =>
      p.hasBackup ? ["restoreBackup", "startFresh", "retry"] : ["retry", "startFresh"],
  },
  other: {
    title: "Sidekick couldn't start",
    body: () => "Something went wrong while starting. Your data hasn't been changed.",
    next: () => "Try again. If it keeps happening, export the logs for a bug report.",
    actions: () => ["retry"],
  },
};

const LABEL: Record<Action, string> = {
  startFresh: "Start fresh…",
  restoreBackup: "Restore backup",
  retry: "Try again",
};

/**
 * Shown instead of the overlay when startup failed (tech doc 4.5): what went
 * wrong, what to do, and the ways out. Starting fresh moves the old data file
 * aside; nothing is deleted.
 */
export function RecoveryScreen({ problem }: { problem: StartupProblem }) {
  const copy = COPY[problem.kind];
  const actions = copy.actions(problem);
  const titleId = useId();
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState<Action | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [exported, setExported] = useState<{ text: string; error: boolean } | null>(null);

  async function run(action: Action) {
    setError(null);
    setBusy(action);
    const r =
      action === "startFresh"
        ? await commands.recoverStartFresh()
        : action === "restoreBackup"
          ? await commands.recoverRestoreBackup()
          : await commands.recoverRetry();
    // On success the app restarts, so only a failure comes back here.
    if (r.status === "error") {
      setError(r.error.message);
      setBusy(null);
    }
  }

  async function exportLogs() {
    setExported(null);
    const path = await chooseLogExportPath();
    if (!path) return;
    const r = await commands.exportStartupLogs(path);
    setExported(
      r.status === "ok"
        ? { text: "Saved the log bundle for a bug report.", error: false }
        : { text: r.error.message, error: true },
    );
  }

  const primary = actions[0];
  return (
    <main
      aria-labelledby={titleId}
      className="flex h-full flex-col gap-3 overflow-auto rounded-xl border border-line bg-surface p-4 text-sm text-fg"
    >
      <div className="flex items-center gap-2">
        <span
          aria-hidden="true"
          className="grid h-5 w-5 shrink-0 place-items-center bg-accent font-display text-sm leading-none text-accent-ink"
        >
          S
        </span>
        <span className="wordmark">Sidekick</span>
      </div>

      <h1 id={titleId} className="heading text-2xl text-balance">
        {copy.title}
      </h1>
      <p className="text-fg-2">{copy.body(problem)}</p>
      {copy.next(problem) && <p className="text-fg-2">{copy.next(problem)}</p>}

      {busy ? (
        <p role="status" className="text-fg">
          Restarting Sidekick…
        </p>
      ) : confirming ? (
        <div
          role="group"
          aria-label="Confirm start fresh"
          className="space-y-2 rounded-lg border border-line bg-popover p-3"
        >
          <p className="font-medium">Start fresh?</p>
          <p className="text-fg-2">
            Sidekick moves the current data file aside and starts with empty data. The old file
            stays in {problem.dataDir}, but Sidekick won&apos;t use it. You&apos;ll add your API
            keys again in Settings.
          </p>
          <div className="flex flex-wrap gap-2">
            <button type="button" className="btn btn-danger" onClick={() => void run("startFresh")}>
              Move it aside and start fresh
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setConfirming(false)}
            >
              Cancel
            </button>
          </div>
        </div>
      ) : (
        <div className="flex flex-wrap gap-2">
          {actions.map((a) => (
            <button
              key={a}
              type="button"
              className={`btn ${a === primary ? "btn-primary" : "btn-secondary"}`}
              onClick={() => (a === "startFresh" ? setConfirming(true) : void run(a))}
            >
              {LABEL[a]}
            </button>
          ))}
          <button type="button" className="btn btn-secondary" onClick={() => void exportLogs()}>
            Export logs…
          </button>
        </div>
      )}

      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
      {exported && (
        <p
          role={exported.error ? "alert" : "status"}
          className={exported.error ? "text-danger" : "text-success"}
        >
          {exported.text}
        </p>
      )}

      <details className="mt-auto text-xs text-fg-muted">
        <summary className="cursor-pointer">Details</summary>
        <p className="mt-1">{problem.message}</p>
        <p className="mt-1 break-all">Data folder: {problem.dataDir}</p>
      </details>
    </main>
  );
}
