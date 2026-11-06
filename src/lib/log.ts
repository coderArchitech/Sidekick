/**
 * Frontend logging (tech doc 7.7): goes to the same log file as Rust via the
 * `log` command. Pass short descriptions and ids only, never content (no
 * transcript, prompts, answers or document text).
 */
import { commands, type LogLevel } from "@/ipc";

interface Ids {
  requestId?: string | null;
  sessionId?: string | null;
}

function send(level: LogLevel, message: string, ids: Ids = {}) {
  void commands
    .log(level, message.slice(0, 500), ids.requestId ?? null, ids.sessionId ?? null)
    .catch(() => {
      // Logging must never break the app.
    });
}

export const log = {
  error: (message: string, ids?: Ids) => send("error", message, ids),
  warn: (message: string, ids?: Ids) => send("warn", message, ids),
  info: (message: string, ids?: Ids) => send("info", message, ids),
};

/** An error's name and first stack frame, without its message (which may hold content). */
export function describeError(e: unknown): string {
  if (e instanceof Error) {
    const frame = e.stack?.split("\n").find((l) => l.trim().startsWith("at ")) ?? "";
    return `${e.name}${frame ? ` ${frame.trim()}` : ""}`;
  }
  return typeof e;
}

let installed = false;

/** Forwards uncaught errors and rejections to the log file. */
export function installErrorLogging(target: Window = window) {
  if (installed) return;
  installed = true;
  target.addEventListener("error", (e) => log.error(`uncaught: ${describeError(e.error)}`));
  target.addEventListener("unhandledrejection", (e) =>
    log.error(`unhandled rejection: ${describeError(e.reason)}`),
  );
}

/** For tests. */
export function resetErrorLogging() {
  installed = false;
}
