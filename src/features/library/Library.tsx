import { useEffect, useId, useState } from "react";
import { useSessionStore } from "@/features/session";
import { onFileDrop, pickFiles, sessionEvents, type DocStatus } from "@/ipc";
import { useLibraryStore } from "./store";

const STATUS: Record<DocStatus, string> = {
  queued: "Waiting",
  parsing: "Reading",
  embedding: "Indexing",
  ready: "Ready",
  failed: "Failed",
};

const button =
  "rounded-md border border-slate-600 px-2 py-0.5 text-xs hover:bg-slate-700 disabled:opacity-50";

/**
 * Session documents (PRD CTX-1, CTX-2): add files with the picker or by
 * dropping them on the window, add links, watch indexing, retry or remove.
 */
export function Library() {
  const session = useSessionStore((s) => s.session);
  const { documents, percent, error, load, addFiles, addUrl, retry, remove, applyProgress } =
    useLibraryStore();
  const [open, setOpen] = useState(false);
  const [url, setUrl] = useState("");
  const urlId = useId();
  const panelId = useId();
  const sessionId = session && session.status !== "ended" ? session.id : null;

  useEffect(() => {
    if (sessionId) void load(sessionId);
  }, [sessionId, load]);

  useEffect(() => {
    const unlisten: (() => void)[] = [];
    let disposed = false;
    const keep = (u: () => void) => (disposed ? u() : unlisten.push(u));
    void sessionEvents.onIngestProgress(applyProgress).then(keep);
    void onFileDrop((paths) => {
      if (useLibraryStore.getState().sessionId) {
        setOpen(true);
        void addFiles(paths);
      }
    }).then(keep);
    return () => {
      disposed = true;
      unlisten.forEach((u) => u());
    };
  }, [applyProgress, addFiles]);

  if (!sessionId) return null;
  const ready = documents.filter((d) => d.status === "ready").length;

  return (
    <div className="border-b border-slate-700 px-3 py-1.5 text-xs">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={panelId}
        className="text-slate-300 hover:text-slate-100"
        onClick={() => setOpen((o) => !o)}
      >
        {open ? "▾" : "▸"} Documents ({ready}/{documents.length} ready)
      </button>
      {open && (
        <div id={panelId} className="mt-2 space-y-2">
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className={button}
              onClick={() => void pickFiles().then(addFiles)}
            >
              Add files
            </button>
            <form
              className="flex min-w-0 flex-1 gap-1"
              onSubmit={(e) => {
                e.preventDefault();
                void addUrl(url).then((ok) => ok && setUrl(""));
              }}
            >
              <label htmlFor={urlId} className="sr-only">
                Link to add
              </label>
              <input
                id={urlId}
                value={url}
                placeholder="https://…"
                onChange={(e) => setUrl(e.target.value)}
                className="min-w-0 flex-1 rounded-md border border-slate-600 bg-slate-950 px-2 py-0.5"
              />
              <button type="submit" className={button} disabled={!url.trim()}>
                Add link
              </button>
            </form>
          </div>
          <p className="text-slate-500">
            Or drop files onto this window. PDF, Word, PowerPoint, Excel, text, Markdown.
          </p>
          {error && (
            <p role="alert" className="text-red-300">
              {error}
            </p>
          )}
          {documents.length > 0 && (
            <ul aria-label="Documents" className="space-y-1">
              {documents.map((d) => {
                const busy = d.status !== "ready" && d.status !== "failed";
                return (
                  <li key={d.id} className="flex items-center gap-2">
                    <span className="min-w-0 flex-1 truncate" title={d.name}>
                      {d.name}
                    </span>
                    <span
                      className={
                        d.status === "ready"
                          ? "text-emerald-300"
                          : d.status === "failed"
                            ? "text-red-300"
                            : "text-amber-300"
                      }
                    >
                      {STATUS[d.status]}
                      {busy && percent[d.id] !== undefined ? ` ${percent[d.id]}%` : ""}
                      {d.status === "ready" && d.pageCount ? ` · ${d.pageCount} p.` : ""}
                    </span>
                    {d.status === "failed" && (
                      <button type="button" className={button} onClick={() => void retry(d)}>
                        Retry
                      </button>
                    )}
                    <button
                      type="button"
                      aria-label={`Remove ${d.name}`}
                      className={button}
                      onClick={() => void remove(d.id)}
                    >
                      ×
                    </button>
                    {d.status === "failed" && d.error && <span className="sr-only">{d.error}</span>}
                  </li>
                );
              })}
            </ul>
          )}
          {documents
            .filter((d) => d.status === "failed" && d.error)
            .map((d) => (
              <p key={`${d.id}-err`} className="text-red-300">
                {d.name}: {d.error}
              </p>
            ))}
        </div>
      )}
    </div>
  );
}
