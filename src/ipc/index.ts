/**
 * The only way the UI talks to the Rust core (tech doc 7.2: components never
 * call `invoke` directly). Exposes the generated, typed commands and events,
 * swapped for mocks when not running inside Tauri.
 */
import type { AnswerEvents } from "./answerEvents";
import { commands as tauriCommands, events as tauriEvents } from "./bindings";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import {
  mockAnswerBus,
  mockCommands,
  mockFiles,
  mockOptions,
  mockSessionBus,
  mockWindowCalls,
} from "./mock";

// In a plain browser (pnpm dev:web) the mocks play a short demo transcript.
if (
  typeof window !== "undefined" &&
  !("__TAURI_INTERNALS__" in window) &&
  import.meta.env.MODE === "web"
) {
  mockOptions.demoTranscript = true;
}
import type { SessionEvents } from "./sessionEvents";

export type { AnswerEvents, Unlisten } from "./answerEvents";
export type { SessionEvents } from "./sessionEvents";
import type { Unlisten } from "./answerEvents";
export type {
  AnswerDone,
  AnswerError,
  AnswerToken,
  Action,
  AppInfo,
  Binding,
  Bookmark,
  ChatMessage,
  ConsentOption,
  ContextChunk,
  ContextOptions,
  ContextPack,
  DocStatus,
  DocumentInfo,
  Diagnostics,
  Effort,
  ErrorCode,
  IngestProgress,
  FinishReason,
  KeyCheck,
  KeyStatus,
  LlmRequest,
  ModelInfo,
  NewAnswer,
  Profile,
  QuestionSource,
  SavedAnswer,
  ProtectionState,
  ProtectionStatus,
  ProviderChoice,
  ProviderEntry,
  ProviderKind,
  Result,
  Retention,
  Session,
  SessionStarted,
  SessionStatus,
  Settings,
  SettingsPatch,
  SidekickError,
  Stage,
  StageStats,
  SttStatusChanged,
  Transcript,
  TranscriptLine,
  TranscriptPartial,
  TranscriptSegment,
  Usage,
} from "./bindings";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const commands: typeof tauriCommands = isTauri ? tauriCommands : mockCommands;

const tauriAnswerEvents: AnswerEvents = {
  onToken: (cb) => tauriEvents.answerToken.listen((e) => cb(e.payload)),
  onDone: (cb) => tauriEvents.answerDone.listen((e) => cb(e.payload)),
  onError: (cb) => tauriEvents.answerError.listen((e) => cb(e.payload)),
};

export const answerEvents: AnswerEvents = isTauri ? tauriAnswerEvents : mockAnswerBus.events;

const tauriSessionEvents: SessionEvents = {
  onSessionChanged: (cb) => tauriEvents.sessionChanged.listen((e) => cb(e.payload)),
  onTranscriptPartial: (cb) => tauriEvents.transcriptPartial.listen((e) => cb(e.payload)),
  onTranscriptSegment: (cb) => tauriEvents.transcriptSegment.listen((e) => cb(e.payload)),
  onSttStatus: (cb) => tauriEvents.sttStatusChanged.listen((e) => cb(e.payload)),
  onHotkey: (cb) => tauriEvents.hotkeyPressed.listen((e) => cb(e.payload)),
  onIngestProgress: (cb) => tauriEvents.ingestProgress.listen((e) => cb(e.payload)),
  onFileDrop: (cb) => onFileDrop(cb),
};

export const sessionEvents: SessionEvents = isTauri ? tauriSessionEvents : mockSessionBus.events;

/** Which window this page is rendering, from the `?window=` query (tech doc 3.7). */
export type WindowKind = "overlay" | "settings";

export function currentWindow(search: string = window.location.search): WindowKind {
  return new URLSearchParams(search).get("window") === "settings" ? "settings" : "overlay";
}

/** Hides the current window (the Settings window closes this way, so it opens fast next time). */
export async function hideCurrentWindow(): Promise<void> {
  if (isTauri) {
    await getCurrentWindow().hide();
  } else {
    mockWindowCalls.hide += 1;
  }
}

/** Files Sidekick can read (PRD CTX-2); images arrive with screenshots. */
export const DOCUMENT_EXTENSIONS = [
  "pdf",
  "docx",
  "pptx",
  "xlsx",
  "xls",
  "ods",
  "txt",
  "md",
  "markdown",
  "csv",
  "json",
];

/** Opens the OS file picker; resolves to the chosen paths (empty if cancelled). */
export async function pickFiles(): Promise<string[]> {
  if (!isTauri) return mockFiles.picked.splice(0);
  const chosen = await open({
    multiple: true,
    filters: [{ name: "Documents", extensions: DOCUMENT_EXTENSIONS }],
  });
  if (!chosen) return [];
  return Array.isArray(chosen) ? chosen : [chosen];
}

/** Files dropped onto the window. */
export function onFileDrop(cb: (paths: string[]) => void): Promise<Unlisten> {
  if (!isTauri) return mockSessionBus.events.onFileDrop(cb);
  return getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === "drop") cb(e.payload.paths);
  });
}
