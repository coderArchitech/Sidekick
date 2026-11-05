/**
 * Answer stream events (tech doc 5.2) behind one small interface, so features
 * don't depend on Tauri's event API and tests can drive them directly.
 */
import type { AnswerDone, AnswerError, AnswerToken } from "./bindings";

export type Unlisten = () => void;

export interface AnswerEvents {
  onToken(cb: (e: AnswerToken) => void): Promise<Unlisten>;
  onDone(cb: (e: AnswerDone) => void): Promise<Unlisten>;
  onError(cb: (e: AnswerError) => void): Promise<Unlisten>;
}

type Handlers = {
  token: Set<(e: AnswerToken) => void>;
  done: Set<(e: AnswerDone) => void>;
  error: Set<(e: AnswerError) => void>;
};

/** In-memory event bus used by the browser mocks and tests. */
export function createMockAnswerBus() {
  const handlers: Handlers = { token: new Set(), done: new Set(), error: new Set() };
  const subscribe = <K extends keyof Handlers>(
    kind: K,
    cb: Handlers[K] extends Set<infer F> ? F : never,
  ): Promise<Unlisten> => {
    (handlers[kind] as Set<typeof cb>).add(cb);
    return Promise.resolve(() => {
      (handlers[kind] as Set<typeof cb>).delete(cb);
    });
  };
  const events: AnswerEvents = {
    onToken: (cb) => subscribe("token", cb),
    onDone: (cb) => subscribe("done", cb),
    onError: (cb) => subscribe("error", cb),
  };
  return {
    events,
    emitToken: (e: AnswerToken) => handlers.token.forEach((h) => h(e)),
    emitDone: (e: AnswerDone) => handlers.done.forEach((h) => h(e)),
    emitError: (e: AnswerError) => handlers.error.forEach((h) => h(e)),
  };
}
