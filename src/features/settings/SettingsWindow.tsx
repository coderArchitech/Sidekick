import { useEffect, useState } from "react";
import {
  commands,
  hideCurrentWindow,
  type Retention,
  type Settings,
  type SettingsPatch,
} from "@/ipc";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { ProfileSection } from "./ProfileSection";
import { ProviderSection } from "./ProviderSection";
import { ShortcutsSection } from "./ShortcutsSection";

const RETENTION_LABELS: Record<Retention, string> = {
  session: "Until the session ends",
  "7d": "7 days",
  "30d": "30 days",
  forever: "Forever",
};

/** The Settings window (tech doc 3.7, PRD 4.9). */
export function SettingsWindow() {
  const [settings, setSettings] = useState<Settings | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void commands.getSettings().then((result) => {
      if (result.status === "ok") setSettings(result.data);
      else setError(result.error.message);
    });
  }, []);

  async function update(patch: SettingsPatch) {
    const result = await commands.updateSettings(patch);
    if (result.status === "ok") {
      setSettings(result.data);
      setError(null);
    } else {
      setError(result.error.message);
    }
  }

  return (
    <main className="min-h-full bg-slate-900 p-6 text-slate-100">
      <div className="mx-auto max-w-2xl space-y-4">
        <header className="flex items-center justify-between">
          <h1 className="text-xl font-semibold">Settings</h1>
          <button
            type="button"
            className="rounded-md border border-slate-600 px-3 py-1 text-sm hover:bg-slate-700"
            onClick={() => void hideCurrentWindow()}
          >
            Close
          </button>
        </header>
        {error && (
          <p role="alert" className="text-sm text-red-300">
            {error}
          </p>
        )}
        {!settings ? (
          <p className="text-sm text-slate-400">Loading…</p>
        ) : (
          <>
            <p className="text-sm text-slate-400">
              Keys are stored in your operating system&apos;s keychain and only sent to the provider
              they belong to.
            </p>
            <ProfileSection
              profile={settings.profile}
              onSave={async (profile) => {
                const result = await commands.updateSettings({ profile });
                if (result.status === "ok") {
                  setSettings(result.data);
                  return { saved: result.data.profile };
                }
                return { error: result.error.message };
              }}
            />
            <ProviderSection
              kind="stt"
              title="Speech-to-text"
              description="Turns the meeting audio into a live transcript."
              choice={settings.stt}
              onChoose={(c) => c && void update({ stt: c })}
            />
            <ProviderSection
              kind="llm"
              title="Answers (LLM)"
              description="Writes cues and full answers."
              choice={settings.llm}
              onChoose={(c) => c && void update({ llm: c })}
            />
            <ProviderSection
              kind="embed"
              title="Document search (embeddings)"
              description="Indexes your documents so answers can cite them. Anthropic has no embeddings service, so pick one of these."
              choice={settings.embeddings}
              onChoose={(c) => c && void update({ embeddings: c })}
            />
            <ProviderSection
              kind="search"
              title="Web search"
              description="Optional. Lets answers use the web when your documents don't have it."
              choice={settings.search}
              allowOff
              onChoose={(c) => void update({ search: c })}
            />
            <section
              aria-labelledby="general-heading"
              className="rounded-lg border border-slate-700 p-4"
            >
              <h2 id="general-heading" className="text-base font-semibold">
                General
              </h2>
              <label className="mt-2 flex items-center gap-2 text-sm">
                <input
                  type="checkbox"
                  checked={settings.autoAnswer}
                  onChange={(e) => void update({ autoAnswer: e.target.checked })}
                />
                Auto-answer every detected question (off: only questions that use your name)
              </label>
              <label className="mt-3 block text-sm text-slate-300" htmlFor="retention">
                Keep session data
              </label>
              <select
                id="retention"
                className="mt-1 rounded-md border border-slate-600 bg-slate-950 px-2 py-1 text-sm"
                value={settings.retention}
                onChange={(e) => void update({ retention: e.target.value as Retention })}
              >
                {(Object.keys(RETENTION_LABELS) as Retention[]).map((r) => (
                  <option key={r} value={r}>
                    {RETENTION_LABELS[r]}
                  </option>
                ))}
              </select>
            </section>
            <ShortcutsSection
              shortcuts={settings.shortcuts}
              onSave={async (shortcuts) => {
                const result = await commands.updateSettings({ shortcuts });
                if (result.status === "ok") {
                  setSettings(result.data);
                  return { saved: result.data.shortcuts };
                }
                return { error: result.error.message };
              }}
            />
            <DiagnosticsPanel />
          </>
        )}
      </div>
    </main>
  );
}
