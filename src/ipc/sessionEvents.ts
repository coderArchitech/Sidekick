/**
 * Session and transcript events (tech doc 5.2) behind a small interface, so
 * features don't depend on Tauri's event API and tests can emit them directly.
 */
import type {
  SessionChanged,
  SttStatusChanged,
  TranscriptPartial,
  TranscriptSegment,
} from "./bindings";
import type { Unlisten } from "./answerEvents";

export interface SessionEvents {
  onSessionChanged(cb: (e: SessionChanged) => void): Promise<Unlisten>;
  onTranscriptPartial(cb: (e: TranscriptPartial) => void): Promise<Unlisten>;
  onTranscriptSegment(cb: (e: TranscriptSegment) => void): Promise<Unlisten>;
  onSttStatus(cb: (e: SttStatusChanged) => void): Promise<Unlisten>;
}

type Payloads = {
  session: SessionChanged;
  partial: TranscriptPartial;
  segment: TranscriptSegment;
  stt: SttStatusChanged;
};

/** In-memory event bus used by the browser mocks and tests. */
export function createMockSessionBus() {
  const handlers: { [K in keyof Payloads]: Set<(e: Payloads[K]) => void> } = {
    session: new Set(),
    partial: new Set(),
    segment: new Set(),
    stt: new Set(),
  };
  function on<K extends keyof Payloads>(kind: K, cb: (e: Payloads[K]) => void): Promise<Unlisten> {
    handlers[kind].add(cb);
    return Promise.resolve(() => {
      handlers[kind].delete(cb);
    });
  }
  function emit<K extends keyof Payloads>(kind: K, e: Payloads[K]) {
    handlers[kind].forEach((h) => h(e));
  }
  const events: SessionEvents = {
    onSessionChanged: (cb) => on("session", cb),
    onTranscriptPartial: (cb) => on("partial", cb),
    onTranscriptSegment: (cb) => on("segment", cb),
    onSttStatus: (cb) => on("stt", cb),
  };
  return { events, emit };
}
