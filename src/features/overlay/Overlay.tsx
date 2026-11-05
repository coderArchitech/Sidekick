import { useEffect, useState } from "react";
import { Library } from "@/features/library";
import { SessionBar } from "@/features/session";
import { TranscriptView } from "@/features/transcript";
import { commands, type AppInfo } from "@/ipc";
import { ProtectionShield } from "./ProtectionShield";
import { useHotkeyActions } from "./useHotkeyActions";
import { MAX_OPACITY, MIN_OPACITY, TABS, useOverlayStore, type Tab } from "./store";

const TAB_LABELS: Record<Tab, string> = {
  answers: "Live Answers",
  transcript: "Transcript",
  notes: "Notes",
  chat: "Chat",
};

/** The floating panel (PRD UI-1, UI-3, UI-7). Tab contents arrive with their features. */
export function Overlay() {
  const { tab, opacity, setTab, setOpacity } = useOverlayStore();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useHotkeyActions();

  useEffect(() => {
    void commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
    });
  }, []);

  return (
    <div
      className="flex h-full flex-col rounded-xl bg-slate-900 text-slate-100 shadow-xl"
      style={{ opacity }}
    >
      <header
        data-tauri-drag-region
        className="flex items-center justify-between gap-2 border-b border-slate-700 px-3 py-2"
      >
        <span className="font-semibold">Sidekick</span>
        <span className="text-xs text-slate-400">{info ? `v${info.version}` : ""}</span>
        <ProtectionShield />
        <button
          type="button"
          className="rounded px-1.5 py-0.5 text-xs text-slate-300 hover:bg-slate-700"
          onClick={() => void commands.openSettings()}
        >
          Settings
        </button>
        <label className="flex items-center gap-1 text-xs text-slate-400">
          Opacity
          <input
            type="range"
            aria-label="Panel opacity"
            min={MIN_OPACITY}
            max={MAX_OPACITY}
            step={0.05}
            value={opacity}
            onChange={(e) => setOpacity(Number(e.target.value))}
          />
        </label>
      </header>

      <SessionBar />
      <Library />

      <div role="tablist" aria-label="Sidekick views" className="flex border-b border-slate-700">
        {TABS.map((id) => (
          <button
            key={id}
            role="tab"
            id={`tab-${id}`}
            aria-selected={tab === id}
            aria-controls={`panel-${id}`}
            className={`flex-1 px-2 py-1.5 text-sm whitespace-nowrap ${tab === id ? "bg-slate-800 font-medium" : "text-slate-400"}`}
            onClick={() => setTab(id)}
          >
            {TAB_LABELS[id]}
          </button>
        ))}
      </div>

      <section
        role="tabpanel"
        id={`panel-${tab}`}
        aria-labelledby={`tab-${tab}`}
        className="flex-1 overflow-auto p-3 text-sm text-slate-300"
      >
        {tab === "transcript" ? <TranscriptView /> : `${TAB_LABELS[tab]} will appear here.`}
      </section>
    </div>
  );
}
