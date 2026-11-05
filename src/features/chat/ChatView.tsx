import { useEffect, useRef, useState, type FormEvent, type KeyboardEvent } from "react";
import { Markdown } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import { activeRouter, askChat, useChatStore, type AnswerView } from "@/orchestrator";
import { CaptureMenu, PendingScreenshot } from "./CaptureMenu";

/** The Chat tab (PRD QA-8): ask your own questions with the session as context. */
export function ChatView() {
  const session = useSessionStore((s) => s.session);
  const turns = useChatStore((s) => s.turns);
  const focusTick = useChatStore((s) => s.focusTick);
  const captureError = useChatStore((s) => s.captureError);
  const pending = useChatStore((s) => s.pending);
  const [text, setText] = useState("");
  const [error, setError] = useState<string | null>(null);
  const input = useRef<HTMLTextAreaElement>(null);
  const end = useRef<HTMLDivElement>(null);
  const canAsk = !!session && session.status !== "ended";

  useEffect(() => {
    if (focusTick > 0) input.current?.focus();
  }, [focusTick]);

  const last = turns[turns.length - 1];
  useEffect(() => {
    end.current?.scrollIntoView?.({ block: "end" });
  }, [turns.length, last?.body]);

  async function send(e?: FormEvent) {
    e?.preventDefault();
    const q = text.trim();
    if (!q || !canAsk) return;
    setText("");
    setError(await askChat(q));
  }

  function onKeyDown(e: KeyboardEvent<HTMLTextAreaElement>) {
    if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
      e.preventDefault();
      void send();
    }
  }

  return (
    <div className="flex h-full flex-col gap-2">
      <div role="log" aria-label="Chat" className="min-h-0 flex-1 space-y-3 overflow-y-auto">
        {turns.length === 0 && (
          <p className="text-slate-400">
            {canAsk
              ? "Ask anything about this meeting or your documents."
              : "Start a session to chat with its context."}
          </p>
        )}
        {turns.map((t) => (
          <ChatTurn key={t.id} turn={t} />
        ))}
        <div ref={end} />
      </div>
      {(error ?? captureError) && (
        <p role="alert" className="text-sm text-red-300">
          {error ?? captureError}
        </p>
      )}
      <PendingScreenshot />
      <form onSubmit={(e) => void send(e)} className="flex items-end gap-2">
        {session && <CaptureMenu sessionId={session.id} disabled={!canAsk} />}
        <textarea
          ref={input}
          aria-label="Ask a question"
          rows={2}
          value={text}
          disabled={!canAsk}
          placeholder={
            !canAsk
              ? ""
              : pending
                ? "What do you want to know about this screenshot?"
                : "Ask a question (Enter to send, Shift+Enter for a new line)"
          }
          onChange={(e) => setText(e.target.value)}
          onKeyDown={onKeyDown}
          className="min-w-0 flex-1 resize-none rounded-md border border-slate-700 bg-slate-950 p-2 text-sm text-slate-100 disabled:opacity-50"
        />
        <button
          type="submit"
          disabled={!canAsk || !text.trim()}
          className="rounded-md bg-sky-700 px-3 py-1.5 text-sm text-white disabled:opacity-40"
        >
          Send
        </button>
      </form>
    </div>
  );
}

function ChatTurn({ turn }: { turn: AnswerView }) {
  const streaming = turn.status === "thinking" || turn.status === "streaming";
  const text = turn.body || turn.cue;
  return (
    <article aria-label={`Chat: ${turn.question}`} className="space-y-1">
      <p className="ml-8 rounded-lg bg-slate-800 px-2 py-1 text-sm whitespace-pre-wrap text-slate-100">
        {turn.question}
      </p>
      <div className="mr-4 text-sm text-slate-200" aria-live="polite">
        {text ? <Markdown text={text} /> : streaming && <p className="text-slate-400">Thinking…</p>}
        {turn.status === "error" && (
          <p role="alert" className="text-red-300">
            {turn.error}
          </p>
        )}
        {turn.status === "cancelled" && <p className="text-xs text-slate-500">Stopped.</p>}
        {streaming && (
          <button
            type="button"
            className="rounded px-1 text-xs text-slate-400 hover:bg-slate-800"
            onClick={() => activeRouter()?.router.cancel(turn.id)}
          >
            Stop
          </button>
        )}
        {turn.status === "done" && turn.sources.length > 0 && (
          <ol aria-label="Sources" className="mt-1 space-y-0.5 text-xs text-slate-400">
            {turn.sources.map((s) => (
              <li key={s.n}>
                [{s.n}] {s.label}
              </li>
            ))}
          </ol>
        )}
      </div>
    </article>
  );
}
