import { create } from "zustand";
import { commands, type DocumentInfo, type IngestProgress } from "@/ipc";

interface LibraryState {
  sessionId: string | null;
  documents: DocumentInfo[];
  /** Live progress percent per document. */
  percent: Record<string, number>;
  /** Where each document came from, so a failed one can be retried. */
  sources: Record<string, { kind: "file" | "url"; value: string }>;
  error: string | null;
  load: (sessionId: string) => Promise<void>;
  addFiles: (paths: string[]) => Promise<void>;
  addUrl: (url: string) => Promise<boolean>;
  retry: (doc: DocumentInfo) => Promise<void>;
  remove: (id: string) => Promise<void>;
  applyProgress: (p: IngestProgress) => void;
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const useLibraryStore = create<LibraryState>((set, get) => {
  async function refresh() {
    const id = get().sessionId;
    if (!id) return;
    const r = await commands.listDocuments(id);
    if (r.status === "ok" && get().sessionId === id) {
      // Remember sources for documents we just queued (matched by name).
      set({ documents: r.data });
    }
  }

  return {
    sessionId: null,
    documents: [],
    percent: {},
    sources: {},
    error: null,

    load: async (sessionId) => {
      set({ sessionId, documents: [], percent: {}, error: null });
      await refresh();
    },

    addFiles: async (paths) => {
      const id = get().sessionId;
      if (!id) return;
      set({ error: null });
      for (const path of paths) {
        const r = await commands.ingestDocument(id, path);
        if (r.status === "error") {
          set({ error: r.error.message });
          continue;
        }
        set((s) => ({
          sources: { ...s.sources, [`name:${fileName(path)}`]: { kind: "file", value: path } },
        }));
      }
      await refresh();
    },

    addUrl: async (url) => {
      const id = get().sessionId;
      if (!id) return false;
      const r = await commands.ingestUrl(id, url.trim());
      if (r.status === "error") {
        set({ error: r.error.message });
        return false;
      }
      set((s) => ({
        error: null,
        sources: { ...s.sources, [`name:${url.trim()}`]: { kind: "url", value: url.trim() } },
      }));
      await refresh();
      return true;
    },

    retry: async (doc) => {
      const source = get().sources[`name:${doc.name}`];
      await get().remove(doc.id);
      if (source?.kind === "file") await get().addFiles([source.value]);
      else if (source?.kind === "url") await get().addUrl(source.value);
    },

    remove: async (docId) => {
      const r = await commands.deleteDocument(docId);
      if (r.status === "error") set({ error: r.error.message });
      set((s) => ({ documents: s.documents.filter((d) => d.id !== docId) }));
    },

    applyProgress: (p) => {
      set((s) => ({ percent: { ...s.percent, [p.documentId]: p.percent } }));
      const known = get().documents.some((d) => d.id === p.documentId);
      if (!known || p.status === "ready" || p.status === "failed") {
        void refresh();
      } else {
        set((s) => ({
          documents: s.documents.map((d) =>
            d.id === p.documentId ? { ...d, status: p.status } : d,
          ),
        }));
      }
    },
  };
});
