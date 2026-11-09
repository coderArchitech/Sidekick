import { useEffect, useState, type CSSProperties } from "react";
import { LiveAnswers } from "@/features/answers";
import { ChatView } from "@/features/chat";
import { Library } from "@/features/library";
import { NotesView } from "@/features/notes";
import { SessionBar, useSessionStore } from "@/features/session";
import { TranscriptView } from "@/features/transcript";
import { commands, type AppInfo } from "@/ipc";
import { useAnswering, useChatStore, useNotes, useQuestionDetection } from "@/orchestrator";
import { ProtectionShield } from "./ProtectionShield";
import { ViewMenu } from "./ViewMenu";
import { useHotkeyActions } from "./useHotkeyActions";
import { TABS, useOverlayStore, type Tab } from "./store";

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

/** The opacity setting tints the background only; text stays opaque (T-049). */
export function glassStyle(opacity: number): CSSProperties {
  return { "--glass-alpha": String(opacity) } as CSSProperties;
}

/** The floating panel (PRD UI-1, UI-3, UI-7). Tab contents arrive with their features. */
export function Overlay() {
  const { tab, opacity, fontSize, compact, setTab, load } = useOverlayStore();
  const [info, setInfo] = useState<AppInfo | null>(null);
  useHotkeyActions();
  const session = useSessionStore((s) => s.session);
  useQuestionDetection(session);
  useAnswering(session);
  useNotes(session);

  useEffect(() => {
    void load();
  }, [load]);

  // A follow-up chip asked in Chat: show it.
  const showTick = useChatStore((s) => s.showTick);
  useEffect(() => {
    if (showTick > 0) setTab("chat");
  }, [showTick, setTab]);

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
      className="glass flex h-full flex-col overflow-hidden rounded-xl border border-line text-fg shadow-2xl shadow-black/40"
      style={glassStyle(opacity)}
      data-compact={compact || undefined}
    >
      <header
        data-tauri-drag-region
        className={`flex items-center gap-2 border-b border-line ${compact ? "px-2 py-1" : "px-3 py-2"}`}
      >
        <span data-tauri-drag-region className="flex min-w-0 items-center gap-2">
          <span
            aria-hidden="true"
            className="grid h-5 w-5 shrink-0 place-items-center rounded-md bg-gradient-to-br from-accent-text to-accent text-[11px] font-bold text-white"
          >
            S
          </span>
          <span className="text-sm font-semibold tracking-tight">Sidekick</span>
          {!compact && info && (
            <span className="text-[11px] whitespace-nowrap text-fg-muted">v{info.version}</span>
          )}
        </span>
        <ProtectionShield />
        <div className="ml-auto flex items-center gap-0.5">
          <ViewMenu />
          <button
            type="button"
            className="btn btn-ghost btn-sm"
            onClick={() => void commands.openSettings()}
          >
            Settings
          </button>
        </div>
      </header>

      {!compact && (
        <>
          <SessionBar />
          <Library />
        </>
      )}

      <div role="tablist" aria-label="Sidekick views" className="flex border-b border-line px-1">
        {TABS.map((id) => (
          <button
            key={id}
            role="tab"
            id={`tab-${id}`}
            aria-selected={tab === id}
            aria-controls={`panel-${id}`}
            className={`relative flex-1 px-2 py-2 text-[0.8125rem] font-medium whitespace-nowrap transition-colors ${
              tab === id
                ? "text-fg after:absolute after:inset-x-3 after:-bottom-px after:h-0.5 after:rounded-full after:bg-accent"
                : "text-fg-muted hover:text-fg-2"
            }`}
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
        className={`flex-1 overflow-auto text-fg-2 ${compact ? "p-2" : "p-3"}`}
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
