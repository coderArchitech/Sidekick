import { createMockAnswerBus } from "./answerEvents";
import { createMockSessionBus } from "./sessionEvents";
import type {
  commands as tauriCommands,
  Bookmark,
  ConsentOption,
  DocumentInfo,
  NewAnswer,
  Session,
  TranscriptLine,
  SidekickError,
  Stage,
  StageStats,
  KeyStatus,
  ProtectionStatus,
  ProviderEntry,
  ProviderKind,
  Settings,
  SettingsPatch,
  Notes,
} from "./bindings";

type Commands = typeof tauriCommands;

/** Same defaults as `Settings::default()` in crates/settings. */
export const DEFAULT_SETTINGS: Settings = {
  stt: { provider: "deepgram", model: "nova-3" },
  llm: { provider: "anthropic", model: null },
  embeddings: { provider: "openai", model: "text-embedding-3-small" },
  search: null,
  autoAnswer: true,
  retention: "30d",
  shortcuts: [
    { action: "show_hide", keys: "CmdOrCtrl+Shift+H" },
    { action: "ask", keys: "CmdOrCtrl+Shift+Space" },
    { action: "screenshot", keys: "CmdOrCtrl+Shift+S" },
    { action: "auto_answer", keys: "CmdOrCtrl+Shift+A" },
    { action: "pause_resume", keys: "CmdOrCtrl+Shift+P" },
    { action: "switch_skill", keys: "CmdOrCtrl+Shift+K" },
    { action: "bookmark", keys: "CmdOrCtrl+Shift+B" },
  ],
  profile: { name: "", nicknames: [] },
};

type CatalogRow = [id: string, kind: ProviderKind, displayName: string, keyRequired: boolean];

/** Mirrors crates/gateway/src/catalog.rs. In mock mode every provider counts as implemented. */
const CATALOG: CatalogRow[] = [
  ["deepgram", "stt", "Deepgram", true],
  ["assemblyai", "stt", "AssemblyAI", true],
  ["soniox", "stt", "Soniox", true],
  ["whisper-local", "stt", "Local Whisper", false],
  ["anthropic", "llm", "Anthropic (Claude)", true],
  ["openai", "llm", "OpenAI", true],
  ["gemini", "llm", "Google Gemini", true],
  ["ollama", "llm", "Ollama (local)", false],
  ["openai", "embed", "OpenAI", true],
  ["gemini", "embed", "Google Gemini", true],
  ["voyage", "embed", "Voyage AI", true],
  ["ollama", "embed", "Ollama (local)", false],
  ["tavily", "search", "Tavily", true],
  ["brave", "search", "Brave Search", true],
];

let settings: Settings = structuredClone(DEFAULT_SETTINGS);
const savedKeys = new Map<string, string>();
const activeStreams = new Map<string, { cancelled: boolean }>();
export const mockAnswerBus = createMockAnswerBus();
export const mockSessionBus = createMockSessionBus();

// ---- Sessions (mirrors crates/session: lifecycle and consent rules) ----------

const sessions = new Map<string, Session>();
const transcripts = new Map<string, TranscriptLine[]>();
const bookmarks = new Map<string, Bookmark[]>();
const notesBySession = new Map<string, Notes>();
const documents = new Map<string, DocumentInfo[]>();
/** Answers saved through `saveAnswer`, for assertions. */
export const savedAnswers: {
  id: string;
  answer: NewAnswer;
  rating: number | null;
  createdAt: number;
}[] = [];

function mockNotes(sessionId: string): Notes {
  return (
    notesBySession.get(sessionId) ?? {
      contentMd: "",
      version: 0,
      updatedAt: 0,
      summaryMd: null,
      actionItems: [],
    }
  );
}

/** Simulates ingest like the Rust pipeline: queued → parsing → embedding → ready (or failed). */
function mockIngest(sessionId: string, name: string, sizeBytes: number) {
  const supported =
    /\.(pdf|docx|pptx|xlsx|xls|ods|txt|md|markdown|csv|json)$/i.test(name) || /^https?:/.test(name);
  if (!supported) {
    return err(
      "UNSUPPORTED_FILE",
      `Sidekick can't read ${name}. Supported: PDF, Word (.docx), PowerPoint (.pptx), Excel, text and Markdown.`,
    );
  }
  const doc: DocumentInfo = {
    id: `mock-doc-${nextLineId++}`,
    name,
    status: "queued",
    pageCount: null,
    chunkCount: 0,
    sizeBytes,
    error: null,
    createdAt: Date.now(),
  };
  documents.set(sessionId, [...(documents.get(sessionId) ?? []), doc]);
  const failing = name.includes("broken");
  const steps: [DocumentInfo["status"], number][] = failing
    ? [
        ["parsing", 5],
        ["failed", 100],
      ]
    : [
        ["parsing", 5],
        ["embedding", 10],
        ["embedding", 60],
        ["ready", 100],
      ];
  steps.forEach(([status, percent], i) =>
    setTimeout(
      () => {
        doc.status = status;
        if (status === "ready") {
          doc.pageCount = 3;
          doc.chunkCount = 3;
        }
        if (status === "failed") doc.error = `Couldn't read ${name}: the PDF is damaged.`;
        mockSessionBus.emit("ingest", { documentId: doc.id, status, percent, error: doc.error });
      },
      (i + 1) * mockOptions.ingestStepMs,
    ),
  );
  return { status: "ok" as const, data: null };
}
let nextLineId = 1;

/** Adds a final transcript line and emits it, like the Rust STT sink does. Tests use it. */
export function mockTranscriptLine(
  sessionId: string,
  line: Omit<TranscriptLine, "id">,
): TranscriptLine {
  const full = { ...line, id: nextLineId++ };
  transcripts.set(sessionId, [...(transcripts.get(sessionId) ?? []), full]);
  mockSessionBus.emit("segment", { sessionId, ...full });
  return full;
}
let liveSessionId: string | null = null;
let nextId = 1;

const err = (code: SidekickError["code"], message: string) => ({
  status: "error" as const,
  error: { code, message, retryable: false },
});

function save(session: Session) {
  sessions.set(session.id, session);
  mockSessionBus.emit("session", { session: structuredClone(session) });
  return { status: "ok" as const, data: structuredClone(session) };
}

const TRANSITIONS: Record<string, Record<string, Session["status"]>> = {
  start: { draft: "live" },
  pause: { live: "paused" },
  resume: { paused: "live" },
  end: { draft: "ended", live: "ended", paused: "ended" },
};

function move(id: string, action: keyof typeof TRANSITIONS) {
  const s = sessions.get(id);
  if (!s) return err("INTERNAL", "That session no longer exists.");
  const next = TRANSITIONS[action]?.[s.status];
  if (!next) return err("INTERNAL", `Can't ${action} a session that is ${s.status}.`);
  const now = Date.now();
  return save({
    ...s,
    status: next,
    startedAt: next === "live" ? (s.startedAt ?? now) : s.startedAt,
    endedAt: next === "ended" ? now : s.endedAt,
  });
}
const STAGE_TARGETS: Record<Stage, [number, number]> = {
  transcript_lag: [1000, 1500],
  question_to_cue: [2000, 3000],
  question_to_answer: [2500, 4000],
  llm_first_token: [900, 2000],
  retrieval: [100, 250],
};
const latencySamples = new Map<Stage, number[]>();

function stageStats(): StageStats[] {
  return (Object.keys(STAGE_TARGETS) as Stage[]).map((stage) => {
    const values = [...(latencySamples.get(stage) ?? [])].sort((a, b) => a - b);
    const pick = (pct: number) =>
      values.length
        ? (values[Math.max(1, Math.ceil((pct * values.length) / 100)) - 1] ?? null)
        : null;
    const [t50, t95] = STAGE_TARGETS[stage];
    const p50 = pick(50);
    const p95 = pick(95);
    return {
      stage,
      count: values.length,
      p50Ms: p50,
      p95Ms: p95,
      targetP50Ms: t50,
      targetP95Ms: t95,
      healthy: p50 === null || p95 === null ? null : p50 <= t50 && p95 <= t95,
    };
  });
}

/** Protection status the mock reports; tests may replace it. */
export const mockProtection: { status: ProtectionStatus } = {
  status: { state: "unavailable", reason: "Running in a browser: no screen-share protection." },
};
/** Paths the mock file picker returns next (tests set them). */
export const mockFiles: { picked: string[] } = { picked: [] };
/** Browser-demo switches; tests leave them off. */
export const mockOptions = { demoTranscript: false, ingestStepMs: 5 };
/** What the mocks were asked to do, for assertions. */
export const mockCalls: { streams: ("mic" | "system")[] } = { streams: [] };
/** Counts window actions so tests can assert them. */
export const mockWindowCalls = { openSettings: 0, hide: 0 };

/** Applies a patch the way the Rust side does: absent fields are unchanged. */
export function applyPatch(current: Settings, patch: SettingsPatch): Settings {
  const next = structuredClone(current);
  if (patch.stt) next.stt = patch.stt;
  if (patch.llm) next.llm = patch.llm;
  if (patch.embeddings) next.embeddings = patch.embeddings;
  if ("search" in patch) next.search = patch.search ?? null;
  if (patch.autoAnswer != null) next.autoAnswer = patch.autoAnswer;
  if (patch.retention) next.retention = patch.retention;
  if (patch.shortcuts) next.shortcuts = patch.shortcuts;
  if (patch.profile) {
    // Mirrors Profile::names: trimmed, no blanks, no duplicates of the name.
    const name = patch.profile.name.trim();
    const seen = new Set(name ? [name.toLowerCase()] : []);
    const nicknames = patch.profile.nicknames
      .map((n) => n.trim())
      .filter((n) => n && !seen.has(n.toLowerCase()) && seen.add(n.toLowerCase()));
    next.profile = { name, nicknames };
  }
  return next;
}

function keyStatus(provider: string): KeyStatus {
  const key = savedKeys.get(provider);
  return key ? { saved: true, last4: key.slice(-4) } : { saved: false, last4: null };
}

function isKnown(provider: string) {
  return CATALOG.some(([id]) => id === provider);
}

const unknownProvider = {
  status: "error" as const,
  error: { code: "INTERNAL" as const, message: "Unknown provider.", retryable: false },
};

/** Resets mock state between tests. */
export function resetMocks(): void {
  settings = structuredClone(DEFAULT_SETTINGS);
  savedKeys.clear();
  activeStreams.clear();
  latencySamples.clear();
  sessions.clear();
  transcripts.clear();
  documents.clear();
  savedAnswers.length = 0;
  mockFiles.picked = [];
  bookmarks.clear();
  notesBySession.clear();
  nextLineId = 1;
  liveSessionId = null;
  nextId = 1;
  mockCalls.streams = [];
  mockWindowCalls.openSettings = 0;
  mockWindowCalls.hide = 0;
  mockProtection.status = {
    state: "unavailable",
    reason: "Running in a browser: no screen-share protection.",
  };
}

/**
 * Deterministic stand-ins for every Rust command, used when the UI runs
 * outside Tauri (`pnpm dev:web`, Vitest, Playwright). Keep in step with
 * bindings.ts: the type annotation makes a missing command a compile error.
 */
export const mockCommands: Commands = {
  appInfo: () =>
    Promise.resolve({
      status: "ok",
      data: { name: "Sidekick", version: "0.0.0-mock", os: "web" },
    }),
  openSettings: () => {
    mockWindowCalls.openSettings += 1;
    return Promise.resolve({ status: "ok", data: null });
  },
  getCaptureProtection: () => Promise.resolve(structuredClone(mockProtection.status)),
  getSettings: () => Promise.resolve({ status: "ok", data: structuredClone(settings) }),
  updateSettings: (patch) => {
    if (patch.shortcuts) {
      // Mirrors sidekick_hotkeys::validate for the cases the UI can produce.
      const norm = patch.shortcuts.map((b) => b.keys.replace(/\s+/g, "").toLowerCase());
      const dup = norm.findIndex((k, i) => norm.indexOf(k) !== i);
      if (dup >= 0)
        return Promise.resolve(err("INTERNAL", `${patch.shortcuts[dup]?.keys} is used twice.`));
      if (norm.some((k) => !k.includes("+"))) {
        return Promise.resolve(
          err("INTERNAL", "Each shortcut needs at least one modifier (Ctrl, Cmd, Alt or Shift)."),
        );
      }
    }
    settings = applyPatch(settings, patch);
    return Promise.resolve({ status: "ok", data: structuredClone(settings) });
  },
  listProviders: (kind) => {
    const data: ProviderEntry[] = CATALOG.filter(([, k]) => k === kind).map(
      ([id, k, displayName, keyRequired]) => ({
        id,
        kind: k,
        displayName,
        keyRequired,
        implemented: true,
        docsUrl: "https://example.com",
        key: keyStatus(id),
      }),
    );
    return Promise.resolve({ status: "ok", data });
  },
  setApiKey: (provider, key) => {
    if (!isKnown(provider)) return Promise.resolve(unknownProvider);
    if (!key.trim()) {
      return Promise.resolve({
        status: "error",
        error: { code: "KEY_MISSING", message: "Paste a key first.", retryable: false },
      });
    }
    // Convention for UI tests: keys containing "invalid" are rejected.
    if (key.includes("invalid")) {
      return Promise.resolve({
        status: "error",
        error: {
          code: "KEY_INVALID",
          message: `${provider} rejected the API key.`,
          retryable: false,
        },
      });
    }
    savedKeys.set(provider, key);
    return Promise.resolve({ status: "ok", data: { key: keyStatus(provider), verified: true } });
  },
  testApiKey: (provider) => {
    if (!savedKeys.has(provider)) {
      return Promise.resolve({
        status: "error",
        error: {
          code: "KEY_MISSING",
          message: `Add your ${provider} API key in Settings.`,
          retryable: false,
        },
      });
    }
    return Promise.resolve({ status: "ok", data: { key: keyStatus(provider), verified: true } });
  },
  deleteApiKey: (provider) => {
    if (!isKnown(provider)) return Promise.resolve(unknownProvider);
    savedKeys.delete(provider);
    return Promise.resolve({ status: "ok", data: null });
  },
  listModels: () =>
    Promise.resolve({
      status: "ok",
      data: [
        { id: "mock-fast", displayName: "Mock (fast)" },
        { id: "mock-large", displayName: "Mock (large)" },
      ],
    }),
  llmStream: (requestId, _provider, request) => {
    const question = [...request.messages].reverse().find((m) => m.role === "user")?.content ?? "";
    const words = `Mock answer to: ${question}`.split(/(?<= )/);
    const handle = { cancelled: false };
    activeStreams.set(requestId, handle);
    void (async () => {
      for (const delta of words) {
        await Promise.resolve();
        if (handle.cancelled) return;
        mockAnswerBus.emitToken({ requestId, delta });
      }
      activeStreams.delete(requestId);
      mockAnswerBus.emitDone({
        requestId,
        usage: { inputTokens: 10, outputTokens: words.length },
        finishReason: "end_turn",
      });
    })();
    return Promise.resolve({ status: "ok", data: null });
  },
  getDiagnostics: () =>
    Promise.resolve({
      appVersion: "0.0.0-mock",
      os: "web",
      protection: structuredClone(mockProtection.status),
      stages: stageStats(),
    }),
  recordLatency: (stage, ms) => {
    latencySamples.set(stage, [...(latencySamples.get(stage) ?? []), ms]);
    return Promise.resolve();
  },
  createSession: (title) => {
    const t = title.trim();
    if (!t || t.length > 200)
      return Promise.resolve(err("INTERNAL", "Give the session a name (up to 200 characters)."));
    const session: Session = {
      id: `mock-session-${nextId++}`,
      title: t,
      status: "draft",
      autoAnswer: settings.autoAnswer,
      createdAt: Date.now(),
      startedAt: null,
      endedAt: null,
      consent: null,
    };
    return Promise.resolve(save(session));
  },
  confirmConsent: (sessionId, option: ConsentOption) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    if (s.status !== "draft")
      return Promise.resolve(err("INTERNAL", "Consent is confirmed before the session starts."));
    return Promise.resolve(save({ ...s, consent: option }));
  },
  startSession: (sessionId) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    if (liveSessionId)
      return Promise.resolve(err("INTERNAL", "End the current session before starting another."));
    if (!s.consent) {
      return Promise.resolve(
        err(
          "CONSENT_REQUIRED",
          'Confirm that participants agree to transcription, or choose "Transcribe me only".',
        ),
      );
    }
    const result = move(sessionId, "start");
    if (result.status === "error") return Promise.resolve(result);
    liveSessionId = sessionId;
    mockCalls.streams = s.consent === "me_only" ? ["mic"] : ["mic", "system"];
    if (mockOptions.demoTranscript) {
      // Browser demo (pnpm dev:web): a partial, then a final line.
      setTimeout(
        () => mockSessionBus.emit("partial", { sessionId, stream: "system", text: "What is the" }),
        300,
      );
      setTimeout(
        () =>
          mockTranscriptLine(sessionId, {
            speakerLabel: "speaker_1",
            isUser: false,
            text: "What is the API rate limit for the Pro tier?",
            startMs: 500,
            endMs: 2600,
          }),
        900,
      );
    }
    return Promise.resolve({ status: "ok", data: { session: result.data, warnings: [] } });
  },
  pauseSession: (sessionId) => Promise.resolve(move(sessionId, "pause")),
  resumeSession: (sessionId) => Promise.resolve(move(sessionId, "resume")),
  endSession: (sessionId) => {
    if (liveSessionId === sessionId) liveSessionId = null;
    return Promise.resolve(move(sessionId, "end"));
  },
  deleteSession: (sessionId) => {
    if (liveSessionId === sessionId) liveSessionId = null;
    sessions.delete(sessionId);
    return Promise.resolve({ status: "ok", data: null });
  },
  setAutoAnswer: (sessionId, on) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    return Promise.resolve(save({ ...s, autoAnswer: on }));
  },
  saveAnswer: (answer) => {
    const id = `mock-answer-${nextLineId++}`;
    savedAnswers.push({ id, answer: structuredClone(answer), rating: null, createdAt: Date.now() });
    return Promise.resolve({ status: "ok", data: id });
  },
  listAnswers: (sessionId, source) =>
    Promise.resolve({
      status: "ok",
      data: savedAnswers
        .filter(
          (a) =>
            a.answer.sessionId === sessionId && (source === null || a.answer.source === source),
        )
        .map((a) => ({
          answerId: a.id,
          question: a.answer.question,
          source: a.answer.source,
          directed: a.answer.directed,
          cue: a.answer.cue,
          bodyMd: a.answer.bodyMd,
          sources: structuredClone(a.answer.sources),
          rating: a.rating,
          createdAt: a.createdAt,
        })),
    }),
  getNotes: (sessionId) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    return Promise.resolve({ status: "ok", data: structuredClone(mockNotes(sessionId)) });
  },
  saveNotes: (sessionId, contentMd, version) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const n = mockNotes(sessionId);
    if (n.version !== version)
      return Promise.resolve(
        err("VERSION_CONFLICT", "The notes changed while they were being updated."),
      );
    const next = { ...n, contentMd, version: n.version + 1, updatedAt: Date.now() };
    notesBySession.set(sessionId, next);
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  saveSummary: (sessionId, summaryMd, actionItems) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const next: Notes = {
      ...mockNotes(sessionId),
      summaryMd,
      actionItems: actionItems
        .filter((a) => a.text.trim() !== "")
        .map((a, i) => ({ ...a, text: a.text.trim(), id: `mock-item-${i}`, done: false })),
    };
    notesBySession.set(sessionId, next);
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  rateAnswer: (answerId, rating) => {
    const a = savedAnswers.find((x) => x.id === answerId);
    if (!a) return Promise.resolve(err("INTERNAL", "That answer no longer exists."));
    a.rating = rating === 0 ? null : rating;
    return Promise.resolve({ status: "ok", data: null });
  },
  getTranscript: (sessionId) =>
    Promise.resolve({
      status: "ok",
      data: {
        lines: structuredClone(transcripts.get(sessionId) ?? []).sort(
          (a, b) => a.startMs - b.startMs || a.id - b.id,
        ),
        bookmarks: structuredClone(bookmarks.get(sessionId) ?? []).sort((a, b) => a.atMs - b.atMs),
      },
    }),
  renameSpeaker: (sessionId, from, to) => {
    const name = to.trim();
    if (!name || name.length > 60)
      return Promise.resolve(err("INTERNAL", "Speaker names need 1 to 60 characters."));
    let changed = 0;
    for (const line of transcripts.get(sessionId) ?? []) {
      if (line.speakerLabel === from) {
        line.speakerLabel = name;
        changed++;
      }
    }
    return Promise.resolve({ status: "ok", data: changed });
  },
  addBookmark: (sessionId, atMs, label) => {
    const mark: Bookmark = {
      id: `mock-bookmark-${nextLineId++}`,
      atMs,
      label: label?.trim() || null,
    };
    bookmarks.set(sessionId, [...(bookmarks.get(sessionId) ?? []), mark]);
    return Promise.resolve({ status: "ok", data: structuredClone(mark) });
  },
  ingestDocument: (sessionId, path) => {
    const name = path.split(/[\\/]/).pop() ?? path;
    return Promise.resolve(mockIngest(sessionId, name, 1000));
  },
  ingestUrl: (sessionId, url) =>
    Promise.resolve(
      /^https?:\/\//.test(url)
        ? mockIngest(sessionId, url, 1000)
        : err("UNSUPPORTED_FILE", "Only http and https links can be added."),
    ),
  retrieveContext: (sessionId, query, options) => {
    const windowMs = options?.windowMs ?? 180_000;
    const lines = transcripts.get(sessionId) ?? [];
    const end = Math.max(0, ...lines.map((l) => l.endMs));
    const words = query
      .toLowerCase()
      .split(/\W+/)
      .filter((w) => w.length > 2);
    const chunks = (documents.get(sessionId) ?? [])
      .filter((d) => d.status === "ready")
      .map((d, i) => ({
        chunkId: i + 1,
        documentId: d.id,
        documentName: d.name,
        page: 1,
        text: `Mock passage from ${d.name} about ${words.join(" ")}`,
      }))
      .slice(0, options?.topK ?? 5);
    return Promise.resolve({
      status: "ok",
      data: {
        transcript: structuredClone(lines.filter((l) => l.endMs >= end - windowMs)),
        notes: null,
        chunks,
      },
    });
  },
  listDocuments: (sessionId) =>
    Promise.resolve({ status: "ok", data: structuredClone(documents.get(sessionId) ?? []) }),
  deleteDocument: (documentId) => {
    for (const [k, list] of documents)
      documents.set(
        k,
        list.filter((d) => d.id !== documentId),
      );
    return Promise.resolve({ status: "ok", data: null });
  },
  listSessions: (_cursor, limit) => {
    const items = [...sessions.values()]
      .sort((a, b) => b.createdAt - a.createdAt || b.id.localeCompare(a.id))
      .slice(0, Math.min(limit ?? 50, 200))
      .map((s) => structuredClone(s));
    return Promise.resolve({ status: "ok", data: { items, nextCursor: null } });
  },
  cancelStream: (requestId) => {
    const handle = activeStreams.get(requestId);
    if (!handle) return Promise.resolve(false);
    handle.cancelled = true;
    activeStreams.delete(requestId);
    mockAnswerBus.emitError({
      requestId,
      error: { code: "CANCELLED", message: "Cancelled.", retryable: false },
    });
    return Promise.resolve(true);
  },
};
