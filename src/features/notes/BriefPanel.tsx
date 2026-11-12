import { useEffect, useState } from "react";
import { Markdown } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import { answerEvents, commands, type Brief } from "@/ipc";
import { modelPlan, sessionPlan, writeBrief } from "@/orchestrator";

/**
 * Pre-meeting brief (PRD CTX-4): a summary of the session's documents and
 * Knowledge Packs, the key numbers and likely questions. Written on request
 * (it costs a model call) and saved with the session.
 */
export function BriefPanel({ sessionId }: { sessionId: string }) {
  const [brief, setBrief] = useState<Brief | null>(null);
  const [open, setOpen] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    void commands.getBrief(sessionId).then((r) => {
      if (live && r.status === "ok") setBrief(r.data);
    });
    return () => {
      live = false;
    };
  }, [sessionId]);

  async function write() {
    setBusy(true);
    setError(null);
    try {
      const s = await commands.getSettings();
      const plan =
        s.status === "ok"
          ? sessionPlan(s.data.llm, useSessionStore.getState().session)
          : modelPlan({ provider: "anthropic", model: null });
      setBrief(await writeBrief({ ...commands, events: answerEvents }, sessionId, plan));
      setOpen(true);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section aria-label="Pre-meeting brief" className="card space-y-2 p-2.5">
      <div className="flex items-center gap-2">
        {brief ? (
          <button
            type="button"
            aria-expanded={open}
            className="text-sm font-semibold text-fg hover:text-accent-text"
            onClick={() => setOpen((o) => !o)}
          >
            {open ? "▾" : "▸"} Pre-meeting brief
          </button>
        ) : (
          <h2 className="heading text-lg text-fg">Pre-meeting brief</h2>
        )}
        <button
          type="button"
          className="btn btn-ghost btn-sm ml-auto"
          disabled={busy}
          onClick={() => void write()}
        >
          {busy ? "Writing…" : brief ? "Rewrite" : "Write brief"}
        </button>
      </div>
      {!brief && !busy && !error && (
        <p className="text-xs text-fg-muted">
          A summary of your documents and Knowledge Packs, the key numbers and likely questions.
        </p>
      )}
      {busy && (
        <p role="status" className="text-xs text-fg-muted">
          Reading your documents…
        </p>
      )}
      {error && (
        <p role="alert" className="text-xs text-warning">
          {error}
        </p>
      )}
      {brief && open && (
        <div className="text-sm text-fg-2 [&_h2]:mt-2 [&_h2]:font-semibold [&_h2]:text-fg">
          <Markdown text={brief.contentMd} />
        </div>
      )}
    </section>
  );
}
