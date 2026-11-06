import { useEffect, useId, useState } from "react";
import { useSessionStore } from "@/features/session";
import { commands, sessionEvents } from "@/ipc";
import { createBatcher } from "@/lib/batcher";
import { bookmarkNow, clock, displayName, useTranscriptStore, type TranscriptEvent } from "./store";

/** The Transcript tab (PRD LIS-3, LIS-6): live lines, partial text, speaker names, bookmarks. */
export function TranscriptView() {
  const session = useSessionStore((s) => s.session);
  const { lines, partials, bookmarks, load, applyEvents, renameLocal } = useTranscriptStore();
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const nameId = useId();
  const sessionId = session?.id;

  // Load what's already saved when the session changes.
  useEffect(() => {
    if (!sessionId) return;
    let cancelled = false;
    void commands.getTranscript(sessionId).then((r) => {
      if (!cancelled && r.status === "ok") load(sessionId, r.data.lines, r.data.bookmarks);
    });
    return () => {
      cancelled = true;
    };
  }, [sessionId, load]);

  // Live events, batched every 50 ms (tech doc 13.5).
  useEffect(() => {
    const batcher = createBatcher<TranscriptEvent>(50, applyEvents);
    const unlisteners: (() => void)[] = [];
    let disposed = false;
    const keep = (u: () => void) => (disposed ? u() : unlisteners.push(u));
    void sessionEvents
      .onTranscriptPartial((event) => batcher.push({ kind: "partial", event }))
      .then(keep);
    void sessionEvents
      .onTranscriptSegment((event) => batcher.push({ kind: "segment", event }))
      .then(keep);
    return () => {
      disposed = true;
      batcher.flushNow();
      unlisteners.forEach((u) => u());
    };
  }, [applyEvents]);

  if (!session) {
    return <p className="text-fg-muted">Start a session to see the transcript.</p>;
  }

  async function rename(from: string) {
    const to = name.trim();
    setEditing(null);
    if (!sessionId || !to || to === from) return;
    const r = await commands.renameSpeaker(sessionId, from, to);
    if (r.status === "ok") {
      renameLocal(from, to);
      setError(null);
    } else {
      setError(r.error.message);
    }
  }

  const live = session.status === "live";
  const partialText = [partials.system, partials.mic].filter(Boolean).join(" … ");

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between text-xs">
        <span role="status" className={live ? "text-danger" : "text-fg-muted"}>
          {live
            ? "● Listening"
            : session.status === "paused"
              ? "Paused: not listening"
              : "Not listening"}
        </span>
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          disabled={!session.startedAt || session.status === "ended"}
          onClick={() => void bookmarkNow(session)}
        >
          Bookmark
        </button>
      </div>
      {error && (
        <p role="alert" className="text-xs text-danger">
          {error}
        </p>
      )}
      {lines.length === 0 && !partialText && (
        <p className="text-fg-muted">{live ? "Waiting for speech…" : "No transcript yet."}</p>
      )}
      <ol aria-label="Transcript" className="space-y-1.5">
        {lines.map((l) => (
          <li key={l.id} className="leading-snug">
            <span className="mr-1 text-xs text-fg-muted">{clock(l.startMs)}</span>
            {editing === l.speakerLabel ? (
              <form
                className="inline"
                onSubmit={(e) => {
                  e.preventDefault();
                  void rename(l.speakerLabel);
                }}
              >
                <label htmlFor={nameId} className="sr-only">
                  New name for {displayName(l.speakerLabel)}
                </label>
                <input
                  id={nameId}
                  value={name}
                  onChange={(e) => setName(e.target.value)}
                  onBlur={() => void rename(l.speakerLabel)}
                  className="field field-sm w-28"
                />
              </form>
            ) : (
              <button
                type="button"
                title="Rename this speaker"
                className={`mr-1 font-medium hover:underline ${l.isUser ? "text-accent-text" : "text-warning"}`}
                onClick={() => {
                  setEditing(l.speakerLabel);
                  setName(displayName(l.speakerLabel));
                }}
              >
                {displayName(l.speakerLabel)}:
              </button>
            )}
            <span>{l.text}</span>
          </li>
        ))}
      </ol>
      {partialText && (
        <p aria-live="polite" className="italic text-fg-muted">
          {partialText}
        </p>
      )}
      {bookmarks.length > 0 && (
        <div>
          <h3 className="text-xs font-semibold text-fg-muted">Bookmarks</h3>
          <ul className="text-xs text-fg-2">
            {bookmarks.map((b) => (
              <li key={b.id}>
                {clock(b.atMs)}
                {b.label ? ` · ${b.label}` : ""}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
