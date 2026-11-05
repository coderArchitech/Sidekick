import { useState } from "react";
import type { Action, Binding } from "@/ipc";

const LABELS: Record<Action, string> = {
  show_hide: "Show / hide overlay",
  ask: "Ask (focus Chat)",
  screenshot: "Screenshot",
  auto_answer: "Auto-answer on/off",
  pause_resume: "Pause / resume listening",
  switch_skill: "Switch Skill",
  bookmark: "Bookmark this moment",
};

interface Props {
  shortcuts: Binding[];
  /** Saves the full set; resolves to the saved (normalised) set, or an error message. */
  onSave: (shortcuts: Binding[]) => Promise<{ saved: Binding[] } | { error: string }>;
}

/** Settings → Shortcuts (tech doc 12.3). `CmdOrCtrl` means Cmd on macOS and Ctrl on Windows. */
export function ShortcutsSection({ shortcuts, onSave }: Props) {
  const [draft, setDraft] = useState<Binding[]>(shortcuts);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const changed = draft.some((b, i) => b.keys !== shortcuts[i]?.keys);

  return (
    <section aria-labelledby="shortcuts-heading" className="rounded-lg border border-slate-700 p-4">
      <h2 id="shortcuts-heading" className="text-base font-semibold">
        Shortcuts
      </h2>
      <p className="mb-3 text-sm text-slate-400">
        Work while another app has focus. Write them like <code>CmdOrCtrl+Shift+H</code> (Cmd on
        macOS, Ctrl on Windows); each needs a modifier.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          void onSave(draft).then((result) => {
            if ("error" in result) {
              setMessage({ ok: false, text: result.error });
            } else {
              setDraft(result.saved);
              setMessage({ ok: true, text: "Shortcuts saved." });
            }
          });
        }}
      >
        <div className="grid grid-cols-[1fr_auto] items-center gap-2 text-sm">
          {draft.map((b, i) => (
            <label key={b.action} className="contents">
              <span>{LABELS[b.action]}</span>
              <input
                value={b.keys}
                spellCheck={false}
                onChange={(e) =>
                  setDraft(draft.map((d, j) => (j === i ? { ...d, keys: e.target.value } : d)))
                }
                className="w-52 rounded-md border border-slate-600 bg-slate-950 px-2 py-1 font-mono text-xs"
              />
            </label>
          ))}
        </div>
        <div className="mt-3 flex gap-2">
          <button
            type="submit"
            disabled={!changed}
            className="rounded-md border border-slate-600 bg-blue-600 px-2.5 py-1 text-sm hover:bg-blue-500 disabled:opacity-50"
          >
            Save shortcuts
          </button>
          <button
            type="button"
            disabled={!changed}
            className="rounded-md border border-slate-600 px-2.5 py-1 text-sm hover:bg-slate-700 disabled:opacity-50"
            onClick={() => {
              setDraft(shortcuts);
              setMessage(null);
            }}
          >
            Undo changes
          </button>
        </div>
      </form>
      {message && (
        <p
          role={message.ok ? "status" : "alert"}
          className={`mt-2 text-sm ${message.ok ? "text-emerald-300" : "text-red-300"}`}
        >
          {message.text}
        </p>
      )}
    </section>
  );
}
