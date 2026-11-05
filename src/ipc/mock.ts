import type { commands as tauriCommands, Settings, SettingsPatch } from "./bindings";

type Commands = typeof tauriCommands;

/** Same defaults as `Settings::default()` in crates/settings. */
export const DEFAULT_SETTINGS: Settings = {
  stt: { provider: "deepgram", model: "nova-3" },
  llm: { provider: "anthropic", model: null },
  embeddings: { provider: "openai", model: "text-embedding-3-small" },
  search: null,
  autoAnswer: true,
  retention: "30d",
};

let settings: Settings = structuredClone(DEFAULT_SETTINGS);

/** Applies a patch the way the Rust side does: absent fields are unchanged. */
export function applyPatch(current: Settings, patch: SettingsPatch): Settings {
  const next = structuredClone(current);
  if (patch.stt) next.stt = patch.stt;
  if (patch.llm) next.llm = patch.llm;
  if (patch.embeddings) next.embeddings = patch.embeddings;
  if ("search" in patch) next.search = patch.search ?? null;
  if (patch.autoAnswer != null) next.autoAnswer = patch.autoAnswer;
  if (patch.retention) next.retention = patch.retention;
  return next;
}

/** Resets mock state between tests. */
export function resetMocks(): void {
  settings = structuredClone(DEFAULT_SETTINGS);
}

/**
 * Deterministic stand-ins for every Rust command, used when the UI runs
 * outside Tauri (`pnpm dev:web`, Vitest, Playwright). Keep in step with
 * bindings.ts: the type annotation makes a missing command a compile error.
 */
export const mockCommands: Commands = {
  appInfo: () =>
    Promise.resolve({
      status: "ok",
      data: { name: "Sidekick", version: "0.0.0-mock", os: "web" },
    }),
  getSettings: () => Promise.resolve({ status: "ok", data: structuredClone(settings) }),
  updateSettings: (patch) => {
    settings = applyPatch(settings, patch);
    return Promise.resolve({ status: "ok", data: structuredClone(settings) });
  },
};
