import { createMockAnswerBus } from "./answerEvents";
import { createMockSessionBus } from "./sessionEvents";
import { createSkillStore, TEMPLATES, toMarkdown, validateSkill } from "./mockSkills";
import type {
  commands as tauriCommands,
  Bookmark,
  ConsentOption,
  Brief,
  DocumentInfo,
  PackInfo,
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
  Screenshot,
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
    { action: "click_through", keys: "CmdOrCtrl+Shift+T" },
  ],
  profile: { name: "", nicknames: [] },
  overlay: { opacity: 80, fontSize: 14, compact: false, positions: [] },
  vocabulary: [],
  theme: "dark",
};

/** Click-through state (Rust keeps it in AppState; never saved). */
let clickThrough = false;

/** Mirrors `clean_vocabulary` in crates/settings: trimmed, no blanks or duplicates. */
export function cleanVocabulary(terms: string[]): string[] {
  const out: string[] = [];
  for (const raw of terms) {
    const t = raw.trim().replace(/\s+/g, " ");
    if (t && !out.some((o) => o.toLowerCase() === t.toLowerCase())) out.push(t);
  }
  return out;
}

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
export const mockSkills = createSkillStore();
/** Mirrors crates/stt/src/models.rs. */
const WHISPER_MODELS = [
  { name: "tiny.en", sizeMb: 75, englishOnly: true },
  { name: "base.en", sizeMb: 142, englishOnly: true },
  { name: "base", sizeMb: 142, englishOnly: false },
  { name: "small", sizeMb: 466, englishOnly: false },
];
const downloadedModels = new Set<string>();
const screenshots: Screenshot[] = [];
/** A 16:9 synthetic "screen" (SVG data URL); the mocks never hold real pixels. */
export const MOCK_IMAGE_URL = `data:image/svg+xml;base64,${btoa(
  '<svg xmlns="http://www.w3.org/2000/svg" width="1600" height="900"><defs><linearGradient id="g" x2="1" y2="1"><stop offset="0" stop-color="#1e293b"/><stop offset="1" stop-color="#0f766e"/></linearGradient></defs><rect width="1600" height="900" fill="url(#g)"/><rect x="120" y="120" width="900" height="560" rx="16" fill="#f8fafc"/><text x="160" y="220" font-size="56" font-family="monospace" fill="#b91c1c">TypeError: x is undefined</text></svg>',
)}`;
const documents = new Map<string, DocumentInfo[]>();
/** Knowledge Packs; their documents live in `documents` under `pack:<id>`. */
const packs = new Map<string, { id: string; name: string; createdAt: number }>();
const sessionPacks = new Map<string, string[]>();
const briefs = new Map<string, Brief>();
let packSeq = 0;
const packKey = (id: string) => `pack:${id}`;

function packInfo(id: string): PackInfo | null {
  const p = packs.get(id);
  if (!p) return null;
  const docs = documents.get(packKey(id)) ?? [];
  return { ...p, documents: docs.length, ready: docs.filter((d) => d.status === "ready").length };
}

/** Ready documents a session can read: its own, then its attached packs' (like Rust). */
function readableDocs(sessionId: string): (DocumentInfo & { packName: string | null })[] {
  return [
    ...(documents.get(sessionId) ?? []).map((d) => ({ ...d, packName: null })),
    ...(sessionPacks.get(sessionId) ?? []).flatMap((id) =>
      (documents.get(packKey(id)) ?? []).map((d) => ({
        ...d,
        packName: packs.get(id)?.name ?? null,
      })),
    ),
  ].filter((d) => d.status === "ready");
}
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
      agenda: [],
    }
  );
}

let agendaSeq = 0;

/** The longest arithmetic run with an operator between digits (like the Rust mock). */
function arithmetic(text: string): string | null {
  let best: string | null = null;
  for (const raw of text.split(/[^0-9 .+\-*/^()%]/)) {
    const run = raw.trim();
    const digits = (run.match(/\d/g) ?? []).length;
    if (digits >= 2 && /[+\-*/^]/.test(run) && (!best || run.length > best.length)) best = run;
  }
  return best;
}

/**
 * Like the Rust mock, once per question: the calculator for arithmetic, web
 * search for "today", "latest", "current" or "now" (when the Skill allows
 * them); otherwise done.
 */
function mockAgentReply(system: string, content: string): string {
  if (content.includes("<tool_results>")) return '{"done": true}';
  const question = /<question>([\s\S]*?)<\/question>/.exec(content)?.[1]?.trim() ?? content;
  const expr = system.includes("- calculator:") ? arithmetic(question) : null;
  if (expr) return JSON.stringify({ tool: "calculator", input: expr });
  const current = question
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .some((w) => ["today", "latest", "current", "now"].includes(w));
  if (system.includes("- web_search:") && current)
    return JSON.stringify({ tool: "web_search", input: question.replace(/"/g, "'") });
  return '{"done": true}';
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

/** Deletes a session and everything in it, like crates/session `repo::delete`. */
function forgetSession(id: string) {
  sessions.delete(id);
  transcripts.delete(id);
  bookmarks.delete(id);
  documents.delete(id);
  notesBySession.delete(id);
  for (let i = screenshots.length - 1; i >= 0; i--)
    if (screenshots[i]?.sessionId === id) screenshots.splice(i, 1);
  for (let i = savedAnswers.length - 1; i >= 0; i--)
    if (savedAnswers[i]?.answer.sessionId === id) savedAnswers.splice(i, 1);
}

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
/** Lines sent to the `log` command, for assertions. */
export const mockLogs: { level: string; message: string; requestId: string | null }[] = [];

export const mockFiles: { picked: string[]; saveAs: string[] } = { picked: [], saveAs: [] };
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
  if (patch.vocabulary) next.vocabulary = cleanVocabulary(patch.vocabulary);
  if (patch.theme) next.theme = patch.theme;
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
  clickThrough = false;
  activeStreams.clear();
  latencySamples.clear();
  sessions.clear();
  transcripts.clear();
  documents.clear();
  savedAnswers.length = 0;
  mockFiles.picked = [];
  mockLogs.length = 0;
  mockFiles.saveAs = [];
  bookmarks.clear();
  notesBySession.clear();
  packs.clear();
  sessionPacks.clear();
  briefs.clear();
  mockSkills.reset();
  downloadedModels.clear();
  screenshots.length = 0;
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
    if (patch.vocabulary) {
      const v = cleanVocabulary(patch.vocabulary);
      if (v.length > 100 || v.some((t) => t.length > 50))
        return Promise.resolve(
          err("INTERNAL", "Custom vocabulary can have up to 100 terms of up to 50 characters."),
        );
    }
    settings = applyPatch(settings, patch);
    // Like Rust: the other window hears about it.
    mockSessionBus.emit("settings", { settings: structuredClone(settings) });
    return Promise.resolve({ status: "ok", data: structuredClone(settings) });
  },
  setClickThrough: (on) => {
    clickThrough = on;
    mockSessionBus.emit("clickThrough", { on });
    return Promise.resolve({ status: "ok", data: on });
  },
  getClickThrough: () => Promise.resolve(clickThrough),
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
    const last = [...request.messages].reverse().find((m) => m.role === "user");
    const question = last?.content ?? "";
    // Mirrors the Rust mock: it notes when a screenshot came with the question.
    const prefix = last?.screenshotId ? "Mock answer (with screenshot) to:" : "Mock answer to:";
    // Like the Rust mock: full answers (answer-sized budgets) cite source [1].
    const cite = request.maxTokens > 100 ? " [1]" : "";
    const agent = request.system?.startsWith("You decide whether tools are needed");
    const words = (
      agent ? mockAgentReply(request.system ?? "", question) : `${prefix} ${question}${cite}`
    ).split(/(?<= )/);
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
  createSession: (title, overrides) => {
    const t = title.trim();
    if (!t || t.length > 200)
      return Promise.resolve(err("INTERNAL", "Give the session a name (up to 200 characters)."));
    // Like Rust: an override must be a known provider of that kind.
    for (const [id, kind] of [
      [overrides?.sttProvider, "stt"],
      [overrides?.llmProvider, "llm"],
    ] as const)
      if (id && !CATALOG.some(([cid, ck]) => cid === id && ck === kind))
        return Promise.resolve(err("INTERNAL", `Sidekick doesn't know the provider ${id}.`));
    const session: Session = {
      id: `mock-session-${nextId++}`,
      title: t,
      status: "draft",
      autoAnswer: settings.autoAnswer,
      createdAt: Date.now(),
      startedAt: null,
      endedAt: null,
      consent: null,
      skillId: null,
      extraSkillIds: [],
      sttProvider: overrides?.sttProvider ?? settings.stt.provider,
      llmProvider: overrides?.llmProvider ?? settings.llm.provider,
      retention: settings.retention,
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
    forgetSession(sessionId);
    return Promise.resolve({ status: "ok", data: null });
  },
  setSessionRetention: (sessionId, retention) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const updated = save({ ...s, retention });
    // "Until the session ends" applies at once to an ended session.
    if (s.status === "ended" && retention === "session") forgetSession(sessionId);
    return Promise.resolve(updated);
  },
  deleteAllData: () => {
    const n = sessions.size;
    liveSessionId = null;
    for (const id of [...sessions.keys()]) forgetSession(id);
    return Promise.resolve({ status: "ok", data: n });
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
  setAgenda: (sessionId, items) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const texts = items.map((i) => i.trim()).filter(Boolean);
    if (texts.length > 30 || texts.some((t) => t.length > 200))
      return Promise.resolve(
        err("INTERNAL", "An agenda can have up to 30 items of up to 200 characters."),
      );
    const n = mockNotes(sessionId);
    // Like Rust: an item already on the agenda keeps its id and status.
    const agenda = texts.map(
      (text) =>
        n.agenda.find((a) => a.text === text) ?? {
          id: `mock-agenda-${++agendaSeq}`,
          text,
          status: "not_started" as const,
        },
    );
    const next = { ...n, agenda };
    notesBySession.set(sessionId, next);
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  updateAgenda: (sessionId, updates) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const n = mockNotes(sessionId);
    const next = {
      ...n,
      agenda: n.agenda.map((a) => {
        const u = updates.find((x) => x.id === a.id);
        return u ? { ...a, status: u.status } : a;
      }),
    };
    notesBySession.set(sessionId, next);
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  setActionItemDone: (sessionId, itemId, done) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const n = mockNotes(sessionId);
    if (!n.actionItems.some((a) => a.id === itemId))
      return Promise.resolve(err("INTERNAL", "That action item no longer exists."));
    const next = {
      ...n,
      actionItems: n.actionItems.map((a) => (a.id === itemId ? { ...a, done } : a)),
    };
    notesBySession.set(sessionId, next);
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  captureScreenshot: (sessionId, request) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    if (s.status === "ended")
      return Promise.resolve(
        err("INTERNAL", "That session has ended. Start a new one to take screenshots."),
      );
    // Same sizes as the Rust MockCapturer after downscaling to 1568 px.
    let size: [number, number] = [1568, 882];
    if (request.mode === "window") {
      if (request.windowId === 1) size = [1200, 800];
      else if (request.windowId === 2) size = [882, 1568];
      else return Promise.resolve(err("INTERNAL", "That window is no longer open."));
    } else if (request.mode === "region") {
      const w = Math.round(2560 * Math.min(1, request.region.width));
      const h = Math.round(1440 * Math.min(1, request.region.height));
      if (w <= 0 || h <= 0) return Promise.resolve(err("INTERNAL", "Select an area to capture."));
      const k = Math.min(1, 1568 / Math.max(w, h));
      size = [Math.round(w * k), Math.round(h * k)];
    }
    const shot: Screenshot = {
      id: `mock-shot-${nextLineId++}`,
      sessionId,
      mimeType: "image/jpeg",
      width: size[0],
      height: size[1],
      createdAt: Date.now(),
    };
    screenshots.push(shot);
    mockSessionBus.emit("screenshot", { screenshot: structuredClone(shot) });
    return Promise.resolve({ status: "ok", data: structuredClone(shot) });
  },
  listWindows: () =>
    Promise.resolve({
      status: "ok",
      data: [
        { id: 1, appName: "Code", title: "main.rs" },
        { id: 2, appName: "Browser", title: "Dashboard" },
      ],
    }),
  previewScreen: () => Promise.resolve({ status: "ok", data: MOCK_IMAGE_URL }),
  listScreenshots: (sessionId) =>
    Promise.resolve({
      status: "ok",
      data: structuredClone(screenshots.filter((s) => s.sessionId === sessionId)),
    }),
  screenshotImage: (id) =>
    Promise.resolve(
      screenshots.some((s) => s.id === id)
        ? { status: "ok", data: MOCK_IMAGE_URL }
        : err("INTERNAL", "That screenshot no longer exists."),
    ),
  deleteScreenshot: (id) => {
    const i = screenshots.findIndex((s) => s.id === id);
    if (i !== -1) screenshots.splice(i, 1);
    return Promise.resolve({ status: "ok", data: null });
  },
  whisperModels: () =>
    Promise.resolve(
      WHISPER_MODELS.map((m) => ({ ...m, downloaded: downloadedModels.has(m.name) })),
    ),
  downloadWhisperModel: async (name) => {
    if (!WHISPER_MODELS.some((m) => m.name === name))
      return err("INTERNAL", `Unknown Whisper model ${name}.`);
    const total = 1_000_000;
    for (const done of [250_000, 500_000, 1_000_000]) {
      await Promise.resolve();
      mockSessionBus.emit("model", { name, doneBytes: done, totalBytes: total });
    }
    downloadedModels.add(name);
    return { status: "ok", data: null };
  },
  listSkills: () => Promise.resolve({ status: "ok", data: mockSkills.list() }),
  skillTemplates: () => Promise.resolve({ status: "ok", data: structuredClone(TEMPLATES) }),
  checkSkill: (skill) => Promise.resolve(validateSkill(skill)),
  saveSkill: (skill) => {
    const r = mockSkills.save(skill);
    if (Array.isArray(r))
      return Promise.resolve(
        err(
          "PARSE_FAILED",
          `This Skill file has problems. ${r.map((e) => `${e.field}: ${e.message}`).join(" ")}`,
        ),
      );
    return Promise.resolve({ status: "ok", data: r });
  },
  setSkillEnabled: (id, enabled) => {
    const s = mockSkills.setEnabled(id, enabled);
    return Promise.resolve(
      s ? { status: "ok", data: s } : err("INTERNAL", "That Skill no longer exists."),
    );
  },
  deleteSkill: (id) => {
    mockSkills.delete(id);
    for (const s of sessions.values())
      if (s.skillId === id || s.extraSkillIds.includes(id))
        save({
          ...s,
          skillId: s.skillId === id ? null : s.skillId,
          extraSkillIds: s.extraSkillIds.filter((x) => x !== id),
        });
    return Promise.resolve({ status: "ok", data: null });
  },
  importSkill: (path) => {
    const p = mockSkills.preview(path);
    if (p === null) return Promise.resolve(err("INTERNAL", "Couldn't open that file."));
    if (Array.isArray(p))
      return Promise.resolve(
        err(
          "PARSE_FAILED",
          `This Skill file has problems. ${p.map((e) => (e.field ? `${e.field}: ${e.message}` : e.message)).join(" ")}`,
        ),
      );
    return Promise.resolve({ status: "ok", data: p });
  },
  exportSkill: (id, path) => {
    const s = mockSkills.get(id);
    if (!s) return Promise.resolve(err("INTERNAL", "That Skill no longer exists."));
    mockSkills.files.set(path, toMarkdown(s));
    return Promise.resolve({ status: "ok", data: null });
  },
  setSessionSkill: (sessionId, skillId) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    if (skillId !== null && !mockSkills.has(skillId))
      return Promise.resolve(err("INTERNAL", "That Skill no longer exists."));
    // Like Rust: the main Skill is never also an extra one.
    return Promise.resolve(
      save({ ...s, skillId, extraSkillIds: s.extraSkillIds.filter((x) => x !== skillId) }),
    );
  },
  setSessionExtraSkills: (sessionId, skillIds) => {
    const s = sessions.get(sessionId);
    if (!s) return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const ids = [...new Set(skillIds)].filter((x) => x !== s.skillId);
    if (ids.length > 2) return Promise.resolve(err("INTERNAL", "Use at most 3 Skills at once."));
    if (ids.some((x) => !mockSkills.has(x)))
      return Promise.resolve(err("INTERNAL", "That Skill no longer exists."));
    return Promise.resolve(save({ ...s, extraSkillIds: ids }));
  },
  setOverlay: (patch) => {
    const o = settings.overlay;
    const next = {
      ...o,
      opacity: patch.opacity ?? o.opacity,
      fontSize: patch.fontSize ?? o.fontSize,
      compact: patch.compact ?? o.compact,
    };
    if (next.opacity < 20 || next.opacity > 100 || next.fontSize < 12 || next.fontSize > 20)
      return Promise.resolve(
        err("INTERNAL", "Opacity must be 20 to 100% and text size 12 to 20 px."),
      );
    settings = { ...settings, overlay: next };
    return Promise.resolve({ status: "ok", data: structuredClone(next) });
  },
  log: (level, message, requestId) => {
    mockLogs.push({ level, message, requestId });
    return Promise.resolve();
  },
  exportLogs: (path) => {
    mockSkills.files.set(path, "zip: diagnostics.json, logs/");
    return Promise.resolve({ status: "ok", data: 1 });
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
    // Like Rust: a chosen screenshot of this session, whatever its age.
    const chosen = options?.screenshotId
      ? screenshots.find((s) => s.id === options.screenshotId && s.sessionId === sessionId)
      : undefined;
    if (options?.screenshotId && !chosen)
      return Promise.resolve(err("INTERNAL", "That screenshot no longer exists."));
    const windowMs = options?.windowMs ?? 180_000;
    const lines = transcripts.get(sessionId) ?? [];
    const end = Math.max(0, ...lines.map((l) => l.endMs));
    const words = query
      .toLowerCase()
      .split(/\W+/)
      .filter((w) => w.length > 2);
    const chunks = readableDocs(sessionId)
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
        notes: notesBySession.get(sessionId)?.contentMd.trim() || null,
        chunks,
        screenshot:
          structuredClone(
            chosen ??
              screenshots
                .filter((s) => s.sessionId === sessionId && s.createdAt >= Date.now() - 600_000)
                .at(-1),
          ) ?? null,
      },
    });
  },
  createPack: (name) => {
    const n = name.trim();
    if (!n || n.length > 100)
      return Promise.resolve(err("INTERNAL", "Give the pack a name (up to 100 characters)."));
    const id = `mock-pack-${++packSeq}`;
    packs.set(id, { id, name: n, createdAt: Date.now() });
    return Promise.resolve({ status: "ok", data: packInfo(id) as PackInfo });
  },
  renamePack: (packId, name) => {
    const p = packs.get(packId);
    const n = name.trim();
    if (!p) return Promise.resolve(err("INTERNAL", "That Knowledge Pack no longer exists."));
    if (!n || n.length > 100)
      return Promise.resolve(err("INTERNAL", "Give the pack a name (up to 100 characters)."));
    p.name = n;
    return Promise.resolve({ status: "ok", data: packInfo(packId) as PackInfo });
  },
  deletePack: (packId) => {
    packs.delete(packId);
    documents.delete(packKey(packId));
    for (const [s, ids] of sessionPacks)
      sessionPacks.set(
        s,
        ids.filter((i) => i !== packId),
      );
    return Promise.resolve({ status: "ok", data: null });
  },
  listPacks: () =>
    Promise.resolve({
      status: "ok",
      data: [...packs.keys()]
        .map((id) => packInfo(id) as PackInfo)
        .sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" })),
    }),
  listPackDocuments: (packId) =>
    Promise.resolve(
      packs.has(packId)
        ? { status: "ok", data: structuredClone(documents.get(packKey(packId)) ?? []) }
        : err("INTERNAL", "That Knowledge Pack no longer exists."),
    ),
  ingestPackDocument: (packId, path) => {
    if (!packs.has(packId))
      return Promise.resolve(err("INTERNAL", "That Knowledge Pack no longer exists."));
    const name = path.split(/[\\/]/).pop() ?? path;
    return Promise.resolve(mockIngest(packKey(packId), name, 1000));
  },
  ingestPackUrl: (packId, url) => {
    if (!packs.has(packId))
      return Promise.resolve(err("INTERNAL", "That Knowledge Pack no longer exists."));
    return Promise.resolve(
      /^https?:\/\//.test(url)
        ? mockIngest(packKey(packId), url, 1000)
        : err("UNSUPPORTED_FILE", "Only http and https links can be added."),
    );
  },
  listSessionPacks: (sessionId) =>
    Promise.resolve({ status: "ok", data: [...(sessionPacks.get(sessionId) ?? [])] }),
  setSessionPacks: (sessionId, packIds) => {
    if (packIds.some((id) => !packs.has(id)))
      return Promise.resolve(err("INTERNAL", "That Knowledge Pack no longer exists."));
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const ids = [...new Set(packIds)].sort((a, b) =>
      (packs.get(a)?.name ?? "").localeCompare(packs.get(b)?.name ?? ""),
    );
    sessionPacks.set(sessionId, ids);
    return Promise.resolve({ status: "ok", data: [...ids] });
  },
  briefMaterial: (sessionId) =>
    Promise.resolve({
      status: "ok",
      data: readableDocs(sessionId).map((d) => ({
        documentName: d.name,
        packName: d.packName,
        text: `Mock content of ${d.name}.`,
      })),
    }),
  getBrief: (sessionId) =>
    Promise.resolve({ status: "ok", data: structuredClone(briefs.get(sessionId) ?? null) }),
  saveBrief: (sessionId, contentMd) => {
    if (!sessions.has(sessionId))
      return Promise.resolve(err("INTERNAL", "That session no longer exists."));
    const b = { contentMd, createdAt: Date.now() };
    briefs.set(sessionId, b);
    return Promise.resolve({ status: "ok", data: structuredClone(b) });
  },
  sessionProviderOptions: () => {
    // Like Rust in mock mode: every provider is ready.
    const of = (kind: ProviderKind) =>
      CATALOG.filter(([, k]) => k === kind).map(([id, , name]) => ({ id, name, ready: true }));
    return Promise.resolve({
      status: "ok",
      data: {
        stt: of("stt"),
        llm: of("llm"),
        defaultStt: settings.stt.provider,
        defaultLlm: settings.llm.provider,
      },
    });
  },
  webSearch: (query) => {
    if (!settings.search)
      return Promise.resolve(
        err("INTERNAL", "Web search is off. Choose Tavily or Brave under Web search in Settings."),
      );
    const q = query.trim().replace(/\s+/g, " ");
    // Like the Rust mock gateway.
    return Promise.resolve({
      status: "ok",
      data: [
        {
          title: `Mock result for ${q}`,
          url: "https://example.com/mock-search",
          snippet: `Mock web snippet about ${q}.`,
        },
      ],
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
