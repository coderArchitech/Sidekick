import { useEffect, useId, useRef, useState } from "react";
import {
  commands,
  sessionEvents,
  type Retention,
  type SessionOverrides,
  type SessionProviderOptions,
} from "@/ipc";
import { Select } from "@/lib/Select";
import { useSkillsStore } from "@/orchestrator";
import { ConsentDialog } from "./ConsentDialog";
import { useSessionStore } from "./store";
import { SttNotices } from "./SttNotices";

const button = "btn btn-secondary btn-sm";

/** Session controls in the overlay: create, start (with consent), pause, resume, end (PRD CTX-1, LIS-6). */
export function SessionBar() {
  const {
    session,
    warnings,
    stt,
    applyStt,
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
  const [overrides, setOverrides] = useState<SessionOverrides>({
    sttProvider: null,
    llmProvider: null,
  });
  const [showOptions, setShowOptions] = useState(false);
  const inputId = useId();

  useEffect(() => {
    const unlisten: (() => void)[] = [];
    let disposed = false;
    const keep = (u: () => void) => (disposed ? u() : unlisten.push(u));
    void sessionEvents.onSessionChanged((e) => applyChange(e.session)).then(keep);
    // Without this, a failed or silent stream looks exactly like a quiet meeting.
    void sessionEvents.onSttStatus(applyStt).then(keep);
    return () => {
      disposed = true;
      unlisten.forEach((u) => u());
    };
  }, [applyChange, applyStt]);

  return (
    <div className="space-y-1.5 border-b border-line px-3 py-2 text-xs">
      {!session || session.status === "ended" ? (
        <form
          className="space-y-1.5"
          onSubmit={(e) => {
            e.preventDefault();
            if (session) reset();
            const picked = overrides.sttProvider || overrides.llmProvider ? overrides : null;
            void create(title, picked).then(() => {
              setTitle("");
              setOverrides({ sttProvider: null, llmProvider: null });
              setShowOptions(false);
            });
          }}
        >
          <div className="flex items-center gap-2">
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
            <button
              type="button"
              aria-expanded={showOptions}
              title="Providers for this session"
              className="btn btn-ghost btn-sm"
              onClick={() => setShowOptions((o) => !o)}
            >
              Options
            </button>
            <button type="submit" className={button} disabled={busy || !title.trim()}>
              New session
            </button>
          </div>
          {showOptions && <ProviderOverrides value={overrides} onChange={setOverrides} />}
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
          <ExtraSkills />
          <button type="button" className={button} disabled={busy} onClick={() => void end()}>
            End
          </button>
        </div>
      )}
      {session?.status === "ended" && <EndedSessionData />}
      {(session?.status === "live" || session?.status === "paused") && <SttNotices stt={stt} />}
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

/**
 * Providers for the next session only (PRD KEY-9): another speech-to-text or
 * answer provider than the Settings defaults. Providers without a key are
 * listed but can't be picked.
 */
function ProviderOverrides({
  value,
  onChange,
}: {
  value: SessionOverrides;
  onChange: (v: SessionOverrides) => void;
}) {
  const [options, setOptions] = useState<SessionProviderOptions | null>(null);
  useEffect(() => {
    let live = true;
    void commands.sessionProviderOptions().then((r) => {
      if (live && r.status === "ok") setOptions(r.data);
    });
    return () => {
      live = false;
    };
  }, []);
  if (!options) return null;
  const pick = (
    label: string,
    list: SessionProviderOptions["stt"],
    def: string,
    current: string | null,
    set: (id: string | null) => void,
  ) => (
    <label className="flex items-center gap-2">
      <span className="w-28 shrink-0 text-fg-muted">{label}</span>
      <Select
        aria-label={`${label} for this session`}
        small
        className="min-w-0 flex-1"
        value={current ?? ""}
        onChange={(v) => set(v || null)}
        options={[
          { value: "", label: `Default (${list.find((p) => p.id === def)?.name ?? def})` },
          ...list
            .filter((p) => p.id !== def)
            .map((p) => ({
              value: p.id,
              label: p.ready ? p.name : `${p.name} (add its key in Settings)`,
              disabled: !p.ready,
            })),
        ]}
      />
    </label>
  );
  return (
    <fieldset className="space-y-1 rounded-md border border-line p-2">
      <legend className="px-1 text-fg-muted">For this session only</legend>
      {pick("Speech-to-text", options.stt, options.defaultStt, value.sttProvider, (id) =>
        onChange({ ...value, sttProvider: id }),
      )}
      {pick("Answers", options.llm, options.defaultLlm, value.llmProvider, (id) =>
        onChange({ ...value, llmProvider: id }),
      )}
    </fieldset>
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
    <Select
      aria-label="Skill"
      title="Skill: how Sidekick answers in this session"
      small
      className="w-32 text-xs"
      value={session.skillId ?? ""}
      disabled={busy}
      onOpen={() => void load()}
      onChange={(v) => void setSkill(v || null)}
      options={[
        { value: "", label: "No Skill" },
        ...enabled.map((s) => ({ value: s.id, label: s.name })),
      ]}
    />
  );
}

/**
 * Skills also in use alongside the main one (PRD SKL-5), up to two. Their
 * instructions are added to the main Skill's; the dropdown and the Switch
 * Skill shortcut still change the main one.
 */
function ExtraSkills() {
  const session = useSessionStore((s) => s.session);
  const setExtraSkills = useSessionStore((s) => s.setExtraSkills);
  const busy = useSessionStore((s) => s.busy);
  const skills = useSkillsStore((s) => s.skills);
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", esc);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", esc);
    };
  }, [open]);
  if (!session || session.status === "ended" || !session.skillId) return null;
  const extra = session.extraSkillIds;
  const others = skills.filter(
    (s) => s.id !== session.skillId && (s.enabled || extra.includes(s.id)),
  );
  if (others.length === 0) return null;
  const full = extra.length >= MAX_EXTRA_SKILLS;
  return (
    <div ref={ref} className="relative">
      <button
        type="button"
        aria-haspopup="dialog"
        aria-expanded={open}
        title="Use more Skills at the same time"
        className="btn btn-ghost btn-sm text-xs"
        disabled={busy}
        onClick={() => setOpen((o) => !o)}
      >
        {extra.length ? `+${extra.length}` : "+"}
        <span className="sr-only"> More Skills</span>
      </button>
      {open && (
        <div
          role="dialog"
          aria-label="Also use"
          className="absolute right-0 z-20 mt-1 w-56 space-y-1 rounded-lg border border-line bg-popover p-2 text-xs text-fg-2 shadow-xl shadow-black/40"
        >
          <p className="text-fg-muted">Also use, with the main Skill (up to {MAX_EXTRA_SKILLS}):</p>
          {others.map((s) => {
            const on = extra.includes(s.id);
            return (
              <label key={s.id} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={on}
                  disabled={busy || (!on && full)}
                  onChange={(e) =>
                    void setExtraSkills(
                      e.target.checked ? [...extra, s.id] : extra.filter((x) => x !== s.id),
                    )
                  }
                />
                <span className="truncate">{s.name}</span>
              </label>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** Matches `MAX_EXTRA_SKILLS` in Rust (`sidekick_session::repo`). */
const MAX_EXTRA_SKILLS = 2;

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
        <Select
          aria-label="Keep this session"
          small
          value={session.retention}
          disabled={busy}
          onChange={(v) => void setRetention(v as Retention)}
          options={(["7d", "30d", "forever"] as Retention[]).map((r) => ({
            value: r,
            label: KEEP_LABELS[r],
          }))}
        />
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
