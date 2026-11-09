import { useEffect, useState } from "react";
import { Select } from "@/lib/Select";
import {
  commands,
  hideCurrentWindow,
  type Retention,
  type Theme,
  type Settings,
  type SettingsPatch,
} from "@/ipc";
import { DataSection } from "./DataSection";
import { DiagnosticsPanel } from "./DiagnosticsPanel";
import { ProfileSection } from "./ProfileSection";
import { ProviderSection } from "./ProviderSection";
import { PacksSection } from "@/features/packs";
import { SkillsSection } from "@/features/skills";
import { ShortcutsSection } from "./ShortcutsSection";
import { VocabularySection } from "./VocabularySection";

const THEME_LABELS: Record<Theme, string> = {
  dark: "Dark",
  light: "Light",
  high_contrast: "High contrast",
  system: "Same as the system",
};

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
    <main className="min-h-full bg-surface p-6 text-fg">
      <div className="mx-auto max-w-2xl space-y-4">
        <header className="flex items-center justify-between">
          <h1 className="text-xl font-semibold">Settings</h1>
          <button
            type="button"
            className="btn btn-secondary"
            onClick={() => void hideCurrentWindow()}
          >
            Close
          </button>
        </header>
        {error && (
          <p role="alert" className="text-sm text-danger">
            {error}
          </p>
        )}
        {!settings ? (
          <p className="text-sm text-fg-muted">Loading…</p>
        ) : (
          <>
            <p className="text-sm text-fg-muted">
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
            <VocabularySection
              vocabulary={settings.vocabulary}
              onSave={async (vocabulary) => {
                const result = await commands.updateSettings({ vocabulary });
                if (result.status === "ok") {
                  setSettings(result.data);
                  return { saved: result.data.vocabulary };
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
            <section aria-labelledby="general-heading" className="card p-4">
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
              <label className="mt-3 block text-sm text-fg-2" htmlFor="retention">
                Keep session data
              </label>
              <Select
                id="retention"
                className="mt-1 max-w-xs"
                value={settings.retention}
                onChange={(v) => void update({ retention: v as Retention })}
                options={(Object.keys(RETENTION_LABELS) as Retention[]).map((r) => ({
                  value: r,
                  label: RETENTION_LABELS[r],
                }))}
              />
              <label className="mt-3 block text-sm text-fg-2" htmlFor="theme">
                Theme
              </label>
              <Select
                id="theme"
                className="mt-1 max-w-xs"
                value={settings.theme}
                onChange={(v) => void update({ theme: v as Theme })}
                options={(Object.keys(THEME_LABELS) as Theme[]).map((t) => ({
                  value: t,
                  label: THEME_LABELS[t],
                }))}
              />
              <p className="mt-1 text-xs text-fg-muted">
                Applies to both windows. High contrast also makes the overlay solid.
              </p>
            </section>
            <SkillsSection />
            <PacksSection />
            <DataSection />
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
