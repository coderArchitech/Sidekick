/**
 * The only way the UI talks to the Rust core (tech doc 7.2: components never
 * call `invoke` directly). Exposes the generated, typed commands, swapped for
 * mocks when not running inside Tauri.
 */
import { commands as tauriCommands } from "./bindings";
import { mockCommands } from "./mock";

export type {
  AppInfo,
  ErrorCode,
  ProviderChoice,
  Result,
  Retention,
  Settings,
  SettingsPatch,
  SidekickError,
} from "./bindings";

export const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export const commands: typeof tauriCommands = isTauri ? tauriCommands : mockCommands;
