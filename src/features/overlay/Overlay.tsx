import { useEffect, useState } from "react";
import { LiveAnswers } from "@/features/answers";
import { ChatView } from "@/features/chat";
import { Library } from "@/features/library";
import { NotesView } from "@/features/notes";
import { SessionBar, useSessionStore } from "@/features/session";
import { TranscriptView } from "@/features/transcript";
import { commands, type AppInfo } from "@/ipc";
import { useAnswering, useNotes, useQuestionDetection } from "@/orchestrator";
import { ProtectionShield } from "./ProtectionShield";
import { useHotkeyActions } from "./useHotkeyActions";
import {
  MAX_FONT,
  MAX_OPACITY,
  MIN_FONT,
  MIN_OPACITY,
  TABS,
  useOverlayStore,
  type Tab,
} from "./store";

const TAB_LABELS: Record<Tab, string> = {
  answers: "Live Answers",
  transcript: "Transcript",
  notes: "Notes",
  chat: "Chat",
};

/** The 14 px default matches the browser's 16 px root, so other sizes scale from it. */
export function rootFontSize(fontSize: number): string {
  return `${(16 * fontSize) / 14}px`;
}

/** The floating panel (PRD UI-1, UI-3, UI-7). Tab contents arrive with their features. */
export function Overlay() {
  const { tab, opacity, fontSize, compact, setTab, setOpacity, setFontSize, setCompact, load } =
    useOverlayStore();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useHotkeyActions();
  const session = useSessionStore((s) => s.session);
  useQuestionDetection(session);
  useAnswering(session);
  useNotes(session);

  useEffect(() => {
    void load();
  }, [load]);

  // Text sizes are rem-based; the overlay has its own webview, so scale the root.
  useEffect(() => {
    document.documentElement.style.fontSize = rootFontSize(fontSize);
  }, [fontSize]);

  useEffect(() => {
    void commands.appInfo().then((result) => {
      if (result.status === "ok") setInfo(result.data);
    });
  }, []);

  return (
    <div
      className="flex h-full flex-col rounded-xl bg-slate-900 text-slate-100 shadow-xl"
      style={{ opacity }}
      data-compact={compact || undefined}
    >
      <header
        data-tauri-drag-region
        className={`flex items-center justify-between gap-1.5 border-b border-slate-700 ${compact ? "px-2 py-1" : "px-3 py-2"}`}
      >
        <span className="font-semibold">Sidekick</span>
        {!compact && (
          <span className="text-xs text-slate-400">{info ? `v${info.version}` : ""}</span>
        )}
        <ProtectionShield />
        <button
          type="button"
          className="rounded px-1.5 py-0.5 text-xs text-slate-300 hover:bg-slate-700"
          onClick={() => void commands.openSettings()}
        >
          Settings
        </button>
        <div className="flex items-center text-xs" role="group" aria-label="Text size">
          <button
            type="button"
            aria-label="Smaller text"
            className="rounded px-1 text-slate-300 hover:bg-slate-700 disabled:opacity-40"
            disabled={fontSize <= MIN_FONT}
            onClick={() => setFontSize(fontSize - 1)}
          >
            A−
          </button>
          <button
            type="button"
            aria-label="Larger text"
            className="rounded px-1 text-slate-300 hover:bg-slate-700 disabled:opacity-40"
            disabled={fontSize >= MAX_FONT}
            onClick={() => setFontSize(fontSize + 1)}
          >
            A+
          </button>
        </div>
        <button
          type="button"
          aria-pressed={compact}
          className={`rounded px-1.5 py-0.5 text-xs hover:bg-slate-700 ${compact ? "text-sky-300" : "text-slate-300"}`}
          title="Compact: hide the session bar and documents"
          onClick={() => setCompact(!compact)}
        >
          Compact
        </button>
        <label className="flex min-w-0 items-center text-xs text-slate-400" title="Opacity">
          <input
            type="range"
            className="w-16 min-w-0"
            aria-label="Panel opacity"
            min={MIN_OPACITY}
            max={MAX_OPACITY}
            step={0.05}
            value={opacity}
            onChange={(e) => setOpacity(Number(e.target.value))}
          />
        </label>
      </header>

      {!compact && (
        <>
          <SessionBar />
          <Library />
        </>
      )}

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
        className={`flex-1 overflow-auto text-slate-300 ${compact ? "p-2" : "p-3"}`}
      >
        {tab === "transcript" ? (
          <TranscriptView />
        ) : tab === "answers" ? (
          <LiveAnswers />
        ) : tab === "chat" ? (
          <ChatView />
        ) : (
          <NotesView />
        )}
      </section>
    </div>
  );
}
