/**
 * Session and transcript events (tech doc 5.2) behind a small interface, so
 * features don't depend on Tauri's event API and tests can emit them directly.
 */
import type {
  HotkeyPressed,
  IngestProgress,
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
  /** A global shortcut was pressed (registered by the Rust shell). */
  onHotkey(cb: (e: HotkeyPressed) => void): Promise<Unlisten>;
  /** Document ingest progress (tech doc 5.2 `ingest.progress`). */
  onIngestProgress(cb: (e: IngestProgress) => void): Promise<Unlisten>;
  /** Mock only: files dropped onto the window (Tauri uses the webview's drop event). */
  onFileDrop(cb: (paths: string[]) => void): Promise<Unlisten>;
}

type Payloads = {
  session: SessionChanged;
  partial: TranscriptPartial;
  segment: TranscriptSegment;
  stt: SttStatusChanged;
  hotkey: HotkeyPressed;
  ingest: IngestProgress;
  drop: string[];
};

/** In-memory event bus used by the browser mocks and tests. */
export function createMockSessionBus() {
  const handlers: { [K in keyof Payloads]: Set<(e: Payloads[K]) => void> } = {
    session: new Set(),
    partial: new Set(),
    segment: new Set(),
    stt: new Set(),
    hotkey: new Set(),
    ingest: new Set(),
    drop: new Set(),
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
    onHotkey: (cb) => on("hotkey", cb),
    onIngestProgress: (cb) => on("ingest", cb),
    onFileDrop: (cb) => on("drop", cb),
  };
  return { events, emit };
}
