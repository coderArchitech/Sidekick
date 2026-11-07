import { useEffect, useId, useState } from "react";
import { sessionEvents, type Retention } from "@/ipc";
import { useSkillsStore } from "@/orchestrator";
import { ConsentDialog } from "./ConsentDialog";
import { useSessionStore } from "./store";

const button = "btn btn-secondary btn-sm";

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
    toggleAuto,
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
    <div className="space-y-1.5 border-b border-line px-3 py-2 text-xs">
      {!session || session.status === "ended" ? (
        <form
          className="flex items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            if (session) reset();
            void create(title).then(() => setTitle(""));
          }}
        >
          {session?.status === "ended" && <span className="text-fg-muted">Session ended.</span>}
          <label htmlFor={inputId} className="sr-only">
            Session name
          </label>
          <input
            id={inputId}
            value={title}
            placeholder="New session name"
            onChange={(e) => setTitle(e.target.value)}
            className="field field-sm min-w-0 flex-1"
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
              <span role="status" className="flex items-center gap-1 text-danger">
                <span
                  aria-hidden="true"
                  className="inline-block h-2 w-2 animate-pulse rounded-full bg-danger"
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
              <span role="status" className="text-warning">
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
          <button
            type="button"
            className={`${button} ${session.autoAnswer ? "border-success text-success" : ""}`}
            aria-pressed={session.autoAnswer}
            title="Auto-answer: on answers every question; off answers only questions that use your name"
            disabled={busy}
            onClick={() => void toggleAuto()}
          >
            Auto
          </button>
          <SkillPicker />
          <button type="button" className={button} disabled={busy} onClick={() => void end()}>
            End
          </button>
        </div>
      )}
      {session?.status === "ended" && <EndedSessionData />}
      {warnings.map((w) => (
        <p key={w} role="status" className="text-warning">
          {w}
        </p>
      ))}
      {error && (
        <p role="alert" className="text-danger">
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

/** The session's active Skill (PRD SKL-1); the list is managed in Settings. */
function SkillPicker() {
  const session = useSessionStore((s) => s.session);
  const setSkill = useSessionStore((s) => s.setSkill);
  const busy = useSessionStore((s) => s.busy);
  const skills = useSkillsStore((s) => s.skills);
  const load = useSkillsStore((s) => s.load);
  useEffect(() => {
    void load();
  }, [load]);
  if (!session || session.status === "ended") return null;
  const enabled = skills.filter((s) => s.enabled || s.id === session.skillId);
  return (
    <select
      aria-label="Skill"
      title="Skill: how Sidekick answers in this session"
      className="max-w-32 field field-sm text-xs"
      value={session.skillId ?? ""}
      disabled={busy}
      onFocus={() => void load()}
      onChange={(e) => void setSkill(e.target.value || null)}
    >
      <option value="">No Skill</option>
      {enabled.map((s) => (
        <option key={s.id} value={s.id}>
          {s.name}
        </option>
      ))}
    </select>
  );
}

const KEEP_LABELS: Record<Retention, string> = {
  session: "Delete now",
  "7d": "7 days",
  "30d": "30 days",
  forever: "Forever",
};

/** After a session ends: how long to keep it, or delete it now (tech doc 4.4). */
function EndedSessionData() {
  const session = useSessionStore((s) => s.session);
  const setRetention = useSessionStore((s) => s.setRetention);
  const remove = useSessionStore((s) => s.remove);
  const busy = useSessionStore((s) => s.busy);
  const [confirming, setConfirming] = useState(false);
  if (!session) return null;
  return (
    <div className="flex items-center gap-2 text-fg-muted">
      <label className="flex items-center gap-1">
        Keep “{session.title}”
        <select
          aria-label="Keep this session"
          className="field field-sm"
          value={session.retention}
          disabled={busy}
          onChange={(e) => void setRetention(e.target.value as Retention)}
        >
          {(["7d", "30d", "forever"] as Retention[]).map((r) => (
            <option key={r} value={r}>
              {KEEP_LABELS[r]}
            </option>
          ))}
        </select>
      </label>
      {confirming ? (
        <>
          <span className="text-warning">Delete transcript, notes, answers and documents?</span>
          <button
            type="button"
            className={`${button} text-danger`}
            disabled={busy}
            onClick={() => void remove()}
          >
            Delete
          </button>
          <button type="button" className={button} onClick={() => setConfirming(false)}>
            Cancel
          </button>
        </>
      ) : (
        <button
          type="button"
          className={button}
          disabled={busy}
          onClick={() => setConfirming(true)}
        >
          Delete session…
        </button>
      )}
    </div>
  );
}
