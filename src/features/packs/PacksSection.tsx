import { useCallback, useEffect, useId, useState } from "react";
import {
  commands,
  pickFiles,
  sessionEvents,
  type DocStatus,
  type DocumentInfo,
  type PackInfo,
} from "@/ipc";

const STATUS: Record<DocStatus, string> = {
  queued: "Waiting",
  parsing: "Reading",
  embedding: "Indexing",
  ready: "Ready",
  failed: "Failed",
};

/**
 * Settings → Knowledge Packs (PRD CTX-5): named sets of documents (product
 * specs, price lists, …) that any session can use. Create, add files or
 * links, remove documents, rename and delete.
 */
export function PacksSection() {
  const headingId = useId();
  const nameId = useId();
  const [packs, setPacks] = useState<PackInfo[]>([]);
  const [name, setName] = useState("");
  const [open, setOpen] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const [version, setVersion] = useState(0);
  const refresh = useCallback(() => setVersion((v) => v + 1), []);

  useEffect(() => {
    let live = true;
    void commands.listPacks().then((r) => {
      if (live && r.status === "ok") setPacks(r.data);
    });
    return () => {
      live = false;
    };
  }, [version]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    void sessionEvents
      .onIngestProgress((p) => {
        if (p.status === "ready" || p.status === "failed") refresh();
      })
      .then((u) => (disposed ? u() : (unlisten = u)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [refresh]);

  async function create(e: React.FormEvent) {
    e.preventDefault();
    const r = await commands.createPack(name);
    if (r.status === "error") {
      setError(r.error.message);
      return;
    }
    setError(null);
    setName("");
    setOpen(r.data.id);
    refresh();
  }

  return (
    <section aria-labelledby={headingId} className="space-y-3">
      <div>
        <h2 id={headingId} className="text-base font-semibold text-fg">
          Knowledge Packs
        </h2>
        <p className="text-sm text-fg-muted">
          Reusable sets of documents, like product specs or price lists. Turn a pack on for a
          session in the overlay's Documents panel.
        </p>
      </div>
      <form onSubmit={(e) => void create(e)} className="flex gap-2">
        <label htmlFor={nameId} className="sr-only">
          New pack name
        </label>
        <input
          id={nameId}
          className="field min-w-0 flex-1"
          placeholder="New pack name, e.g. Product specs"
          value={name}
          onChange={(e) => setName(e.target.value)}
        />
        <button type="submit" className="btn btn-primary" disabled={!name.trim()}>
          Create pack
        </button>
      </form>
      {error && (
        <p role="alert" className="text-sm text-danger">
          {error}
        </p>
      )}
      {packs.length === 0 ? (
        <p className="text-sm text-fg-muted">No packs yet.</p>
      ) : (
        <ul aria-label="Knowledge Packs" className="space-y-2">
          {packs.map((p) => (
            <PackRow
              key={p.id}
              pack={p}
              open={open === p.id}
              onToggle={() => setOpen(open === p.id ? null : p.id)}
              onChanged={refresh}
            />
          ))}
        </ul>
      )}
    </section>
  );
}

function PackRow({
  pack,
  open,
  onToggle,
  onChanged,
}: {
  pack: PackInfo;
  open: boolean;
  onToggle: () => void;
  onChanged: () => void;
}) {
  const panelId = useId();
  const urlId = useId();
  const [docs, setDocs] = useState<DocumentInfo[]>([]);
  const [url, setUrl] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const [renaming, setRenaming] = useState<string | null>(null);

  const [docsVersion, setDocsVersion] = useState(0);

  // Reload when opened, after a change, and when the pack's counts change
  // (ingest progress refreshes the pack list).
  useEffect(() => {
    if (!open) return;
    let live = true;
    void commands.listPackDocuments(pack.id).then((r) => {
      if (live && r.status === "ok") setDocs(r.data);
    });
    return () => {
      live = false;
    };
  }, [open, pack.id, pack.documents, pack.ready, docsVersion]);

  async function run(
    r: Promise<{ status: "ok" } | { status: "error"; error: { message: string } }>,
  ) {
    const res = await r;
    setError(res.status === "error" ? res.error.message : null);
    setDocsVersion((v) => v + 1);
    onChanged();
  }

  return (
    <li className="card p-3">
      <div className="flex items-center gap-2">
        <button
          type="button"
          aria-expanded={open}
          aria-controls={panelId}
          className="min-w-0 flex-1 truncate text-left text-sm font-medium text-fg hover:text-accent-text"
          onClick={onToggle}
        >
          {open ? "▾" : "▸"} {pack.name}
        </button>
        <span className="text-xs text-fg-muted tabular">
          {pack.ready}/{pack.documents} ready
        </span>
      </div>
      {open && (
        <div id={panelId} className="mt-3 space-y-2 text-sm">
          <div className="flex flex-wrap gap-2">
            <button
              type="button"
              className="btn btn-secondary btn-sm"
              onClick={() =>
                void pickFiles().then(async (paths) => {
                  for (const p of paths) await run(commands.ingestPackDocument(pack.id, p));
                })
              }
            >
              Add files
            </button>
            <form
              className="flex min-w-0 flex-1 gap-1"
              onSubmit={(e) => {
                e.preventDefault();
                void run(commands.ingestPackUrl(pack.id, url.trim())).then(() => setUrl(""));
              }}
            >
              <label htmlFor={urlId} className="sr-only">
                Link to add to {pack.name}
              </label>
              <input
                id={urlId}
                className="field field-sm min-w-0 flex-1"
                placeholder="https://…"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
              />
              <button type="submit" className="btn btn-secondary btn-sm" disabled={!url.trim()}>
                Add link
              </button>
            </form>
          </div>
          {error && (
            <p role="alert" className="text-danger">
              {error}
            </p>
          )}
          {docs.length > 0 && (
            <ul aria-label={`Documents in ${pack.name}`} className="space-y-1">
              {docs.map((d) => (
                <li key={d.id} className="flex items-center gap-2 text-xs">
                  <span className="min-w-0 flex-1 truncate" title={d.name}>
                    {d.name}
                  </span>
                  <span
                    className={
                      d.status === "ready"
                        ? "text-success"
                        : d.status === "failed"
                          ? "text-danger"
                          : "text-warning"
                    }
                  >
                    {STATUS[d.status]}
                  </span>
                  <button
                    type="button"
                    aria-label={`Remove ${d.name}`}
                    className="btn btn-ghost btn-sm"
                    onClick={() => void run(commands.deleteDocument(d.id))}
                  >
                    ×
                  </button>
                </li>
              ))}
            </ul>
          )}
          <div className="flex flex-wrap items-center gap-2 border-t border-line pt-2">
            {renaming === null ? (
              <button
                type="button"
                className="btn btn-ghost btn-sm"
                onClick={() => setRenaming(pack.name)}
              >
                Rename
              </button>
            ) : (
              <form
                className="flex gap-1"
                onSubmit={(e) => {
                  e.preventDefault();
                  void run(commands.renamePack(pack.id, renaming)).then(() => setRenaming(null));
                }}
              >
                <input
                  aria-label={`New name for ${pack.name}`}
                  className="field field-sm"
                  value={renaming}
                  onChange={(e) => setRenaming(e.target.value)}
                />
                <button type="submit" className="btn btn-secondary btn-sm">
                  Save name
                </button>
              </form>
            )}
            {confirming ? (
              <>
                <span className="text-xs text-fg-muted">Delete the pack and its documents?</span>
                <button
                  type="button"
                  className="btn btn-danger btn-sm"
                  onClick={() => void run(commands.deletePack(pack.id))}
                >
                  Delete {pack.name}
                </button>
                <button
                  type="button"
                  className="btn btn-ghost btn-sm"
                  onClick={() => setConfirming(false)}
                >
                  Keep
                </button>
              </>
            ) : (
              <button
                type="button"
                className="btn btn-danger-ghost btn-sm ml-auto"
                onClick={() => setConfirming(true)}
              >
                Delete pack
              </button>
            )}
          </div>
        </div>
      )}
    </li>
  );
}
