import type { commands as tauriCommands } from "./bindings";

type Commands = typeof tauriCommands;

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
};
