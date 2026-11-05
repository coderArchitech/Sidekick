/**
 * The only way the UI talks to the Rust core (tech doc 7.2: components never
 * call `invoke` directly). Exposes the generated, typed commands and events,
 * swapped for mocks when not running inside Tauri.
 */
import type { AnswerEvents } from "./answerEvents";
import { commands as tauriCommands, events as tauriEvents } from "./bindings";
import { mockAnswerBus, mockCommands } from "./mock";

export type { AnswerEvents, Unlisten } from "./answerEvents";
export type {
  AnswerDone,
  AnswerError,
  AnswerToken,
  AppInfo,
  ChatMessage,
  Effort,
  ErrorCode,
  FinishReason,
  KeyCheck,
  KeyStatus,
  LlmRequest,
  ModelInfo,
  ProviderChoice,
  ProviderEntry,
  ProviderKind,
  Result,
  Retention,
  Settings,
  SettingsPatch,
  SidekickError,
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
