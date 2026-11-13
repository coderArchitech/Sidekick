/**
 * The only way the UI talks to the Rust core (tech doc 7.2: components never
 * call `invoke` directly). Exposes the generated, typed commands and events,
 * swapped for mocks when not running inside Tauri.
 */
import type { AnswerEvents } from "./answerEvents";
import { commands as tauriCommands, events as tauriEvents } from "./bindings";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  mockAnswerBus,
  mockCommands,
  mockFiles,
  mockOptions,
  mockProtection,
  mockSessionBus,
  mockSkills,
  mockTranscriptLine,
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

// Playwright UI tests (vite --mode uitest, T-032) drive the mocked backend
// through this hook: emit events, add transcript lines, queue file picks.
if (
  typeof window !== "undefined" &&
  !("__TAURI_INTERNALS__" in window) &&
  import.meta.env.MODE === "uitest"
) {
  // Options set before the app loads (page.addInitScript), e.g. the README
  // screenshots' canned replies and version.
  const preset = (
    window as unknown as {
      __sidekickMockOptions?: Partial<typeof mockOptions> & {
        protection?: typeof mockProtection.status;
      };
    }
  ).__sidekickMockOptions;
  if (preset) {
    const { protection, ...options } = preset;
    Object.assign(mockOptions, options);
    if (protection) mockProtection.status = protection;
  }
  (window as unknown as { __sidekickTest: unknown }).__sidekickTest = {
    commands: mockCommands,
    sessionBus: mockSessionBus,
    transcriptLine: mockTranscriptLine,
    files: mockFiles,
    skills: mockSkills,
    protection: mockProtection,
    options: mockOptions,
  };
}
import type { SessionEvents } from "./sessionEvents";

export type { AnswerEvents, Unlisten } from "./answerEvents";
export type { SessionEvents } from "./sessionEvents";
import type { Unlisten } from "./answerEvents";
export type {
  ActionItem,
  AgendaItem,
  Brief,
  BriefSource,
  AgendaStatus,
  AgendaUpdate,
  AnswerFormat,
  FieldError,
  ImportPreview,
  Skill,
  StartupProblem,
  ProblemKind,
  SkillEntry,
  Tool,
  Trigger,
  AnswerDone,
  AnswerError,
  AnswerToken,
  Action,
  AppInfo,
  Binding,
  Bookmark,
  CaptureRequest,
  ChatMessage,
  ConsentOption,
  ContextChunk,
  ContextOptions,
  ContextPack,
  DocStatus,
  DocumentInfo,
  PackInfo,
  Diagnostics,
  Effort,
  ErrorCode,
  IngestProgress,
  ModelDownloadProgress,
  FinishReason,
  KeyCheck,
  KeyStatus,
  LogLevel,
  LlmRequest,
  ModelInfo,
  NewActionItem,
  NewAnswer,
  OverlayPatch,
  OverlayPrefs,
  Notes,
  Profile,
  QuestionSource,
  SavedAnswer,
  Screenshot,
  ScreenshotCaptured,
  ProtectionState,
  ProtectionStatus,
  ProviderChoice,
  ProviderEntry,
  ProviderKind,
  Region,
  Result,
  Retention,
  Session,
  Theme,
  SessionOverrides,
  SessionProviderOptions,
  SearchResult,
  SessionStarted,
  SessionStatus,
  Settings,
  SettingsPatch,
  SidekickError,
  Stage,
  StageStats,
  SttStatusChanged,
  SttState,
  Stream,
  Transcript,
  TranscriptLine,
  TranscriptPartial,
  TranscriptSegment,
  Usage,
  WhisperModelEntry,
  WindowInfo,
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
  onScreenshotCaptured: (cb) => tauriEvents.screenshotCaptured.listen((e) => cb(e.payload)),
  onModelDownloadProgress: (cb) => tauriEvents.modelDownloadProgress.listen((e) => cb(e.payload)),
  onSettingsChanged: (cb) => tauriEvents.settingsChanged.listen((e) => cb(e.payload)),
  onClickThroughChanged: (cb) => tauriEvents.clickThroughChanged.listen((e) => cb(e.payload)),
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

/** Opens a Skill file to import (Settings window). Null if cancelled. */
export async function pickSkillFile(): Promise<string | null> {
  if (!isTauri) return mockFiles.picked.shift() ?? null;
  const chosen = await open({
    multiple: false,
    filters: [{ name: "Skill", extensions: ["md", "markdown"] }],
  });
  return typeof chosen === "string" ? chosen : null;
}

/** Asks where to export a Skill. Null if cancelled. */
export async function chooseSkillExportPath(defaultName: string): Promise<string | null> {
  if (!isTauri) return mockFiles.saveAs.shift() ?? null;
  return save({ defaultPath: defaultName, filters: [{ name: "Skill", extensions: ["md"] }] });
}

/** Asks where to save the log bundle. Null if cancelled. */
export async function chooseLogExportPath(): Promise<string | null> {
  if (!isTauri) return mockFiles.saveAs.shift() ?? null;
  const day = new Date().toISOString().slice(0, 10);
  return save({
    defaultPath: `sidekick-logs-${day}.zip`,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
}

/** Files dropped onto the window. */
export function onFileDrop(cb: (paths: string[]) => void): Promise<Unlisten> {
  if (!isTauri) return mockSessionBus.events.onFileDrop(cb);
  return getCurrentWebview().onDragDropEvent((e) => {
    if (e.payload.type === "drop") cb(e.payload.paths);
  });
}
