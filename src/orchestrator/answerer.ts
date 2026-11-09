import type {
  AnswerEvents,
  ContextOptions,
  ContextPack,
  LlmRequest,
  NewAnswer,
  QuestionSource,
  Result,
  SidekickError,
  Stage,
} from "@/ipc";
import { log } from "@/lib/log";
import { runAgent, type AgentDeps, type ToolResult } from "./agent";
import type { ModelPlan } from "./models";
import {
  NOT_FOUND,
  buildPrompt,
  citedSources,
  isEmpty,
  type SkillPrompt,
  type SourceRef,
} from "./prompt";

/** What the router needs from the Rust core; real `commands` fit, tests pass fakes. */
export interface AnswerDeps {
  retrieveContext(
    sessionId: string,
    query: string,
    options: ContextOptions | null,
  ): Promise<Result<ContextPack, SidekickError>>;
  llmStream(
    requestId: string,
    provider: string,
    request: LlmRequest,
  ): Promise<Result<null, SidekickError>>;
  cancelStream(requestId: string): Promise<boolean>;
  saveAnswer(answer: NewAnswer): Promise<Result<string, SidekickError>>;
  recordLatency(stage: Stage, ms: number): Promise<void>;
  events: AnswerEvents;
  now?: () => number;
  /** Web search for agent Skills (T-046). */
  webSearch?: AgentDeps["webSearch"];
}

export type AnswerStatus = "thinking" | "streaming" | "done" | "error" | "cancelled";

export interface AnswerView {
  id: string;
  question: string;
  directed: boolean;
  source: QuestionSource;
  cue: string;
  body: string;
  sources: SourceRef[];
  status: AnswerStatus;
  error: string | null;
  /** Saved answer id, for rating. */
  answerId: string | null;
  /** Tools an agent Skill ran for this answer (PRD SKL-4). */
  tools?: ToolResult[];
  /** Shown in Live Answers (detected questions, or a screenshot taken there). */
  live?: boolean;
}

export interface AskInput {
  sessionId: string;
  text: string;
  directed: boolean;
  source: QuestionSource;
  segmentId?: number | null;
  /** When the question ended (epoch ms), for latency (tech doc 13.1). */
  endedAt: number;
  models: ModelPlan;
  skill?: SkillPrompt | null;
  history?: LlmRequest["messages"];
  /** Shown and saved as the question instead of `text` (follow-up chips, PRD QA-9). */
  label?: string;
  /** Searched in the documents instead of `text`. */
  query?: string;
  /** Answer about this screenshot (quick actions, re-ask; PRD SCR-5, SCR-6). */
  screenshotId?: string;
  /** Show it in Live Answers even though the user asked (a screenshot taken there). */
  live?: boolean;
}

interface InFlight {
  view: AnswerView;
  requestIds: string[];
  cueTokens: string;
  bodyTokens: string;
  started: number;
  endedAt: number;
  sawCue: boolean;
  sawBody: boolean;
  cueDone: boolean;
  bodyDone: boolean;
  usage: { inputTokens: number; outputTokens: number } | null;
  input: AskInput;
  sources: SourceRef[];
}

let counter = 0;

/**
 * Answers questions with a fast cue and a full answer streamed in parallel
 * (tech doc 3.3). At most `maxInFlight` questions run at once; a new one
 * cancels the oldest (tech doc 5.5). A failure never throws: it shows on that
 * answer only, so transcript and notes keep working (NFR-5).
 */
export class AnswerRouter {
  private readonly inFlight = new Map<string, InFlight>();
  private readonly byRequest = new Map<string, { id: string; part: "cue" | "full" }>();
  private readonly unlisten: (() => void)[] = [];
  private readonly now: () => number;

  constructor(
    private readonly deps: AnswerDeps,
    private readonly onUpdate: (view: AnswerView) => void,
    private readonly maxInFlight = 2,
  ) {
    this.now = deps.now ?? Date.now;
    void deps.events
      .onToken((e) => this.onToken(e.requestId, e.delta))
      .then((u) => this.unlisten.push(u));
    void deps.events
      .onDone((e) => this.onDone(e.requestId, e.usage, e.finishReason))
      .then((u) => this.unlisten.push(u));
    void deps.events
      .onError((e) => this.onError(e.requestId, e.error))
      .then((u) => this.unlisten.push(u));
  }

  dispose() {
    this.unlisten.forEach((u) => u());
    for (const f of this.inFlight.values())
      f.requestIds.forEach((r) => void this.deps.cancelStream(r));
    this.inFlight.clear();
  }

  async ask(input: AskInput): Promise<AnswerView> {
    const id = `a${Date.now().toString(36)}${(counter++).toString(36)}`;
    const view: AnswerView = {
      id,
      question: input.label ?? input.text,
      directed: input.directed,
      ...(input.live ? { live: true } : {}),
      source: input.source,
      cue: "",
      body: "",
      sources: [],
      status: "thinking",
      error: null,
      answerId: null,
    };
    this.onUpdate({ ...view });
    // Make room: the newest question matters most.
    while (this.inFlight.size >= this.maxInFlight) {
      const oldest = this.inFlight.keys().next().value;
      if (oldest === undefined) break;
      this.cancel(oldest);
    }
    const flight: InFlight = {
      view,
      requestIds: [],
      cueTokens: "",
      bodyTokens: "",
      started: this.now(),
      endedAt: input.endedAt,
      sawCue: false,
      sawBody: false,
      cueDone: false,
      bodyDone: false,
      usage: null,
      input,
      sources: [],
    };
    this.inFlight.set(id, flight);

    const ctx = await this.deps.retrieveContext(
      input.sessionId,
      input.query ?? input.text,
      // The defaults match ContextOptions::default() in Rust.
      input.screenshotId ? { windowMs: 180_000, topK: 5, screenshotId: input.screenshotId } : null,
    );
    if (!this.inFlight.has(id)) return view; // cancelled meanwhile
    if (ctx.status === "error") return this.fail(flight, ctx.error.message);
    // An agent Skill may use its tools first (PRD SKL-4).
    let toolResults: ToolResult[] = [];
    if (input.skill?.tools?.length) {
      flight.view = { ...flight.view, tools: [] };
      toolResults = await runAgent(
        {
          llmStream: (rid, provider, req) => this.deps.llmStream(rid, provider ?? "", req),
          events: this.deps.events,
          retrieveContext: this.deps.retrieveContext.bind(this.deps),
          webSearch: this.deps.webSearch,
          now: this.now,
        },
        {
          sessionId: input.sessionId,
          question: input.label ?? input.text,
          tools: input.skill.tools,
          plan: input.models,
        },
      );
      if (!this.inFlight.has(id)) return view; // cancelled meanwhile
      flight.view = { ...flight.view, tools: toolResults };
      this.onUpdate({ ...flight.view });
    }
    if (isEmpty(ctx.data) && !toolResults.some((r) => r.ok)) {
      // Nothing to answer from: say so, never guess (QA-6).
      flight.view = { ...flight.view, cue: NOT_FOUND, body: NOT_FOUND, status: "done" };
      this.inFlight.delete(id);
      this.onUpdate({ ...flight.view });
      return flight.view;
    }
    const full = buildPrompt(
      {
        question: input.text,
        pack: ctx.data,
        skill: input.skill,
        history: input.history,
        toolResults,
      },
      "full",
    );
    const cue = buildPrompt(
      {
        question: input.text,
        pack: ctx.data,
        skill: input.skill,
        screenshotFocus: input.source === "screenshot",
        toolResults,
      },
      "cue",
    );
    flight.sources = full.sources;
    const plan = input.models;
    const allRequests: [string, "cue" | "full", LlmRequest][] = [
      [
        `${id}:cue`,
        "cue",
        {
          model: plan.cue,
          system: cue.system,
          messages: cue.messages,
          maxTokens: 80,
          effort: "low",
        },
      ],
      [
        `${id}:full`,
        "full",
        {
          model: plan.full,
          system: full.system,
          messages: full.messages,
          maxTokens: 1500,
          effort: "medium",
        },
      ],
    ];
    // A Skill can ask for the full answer only (no one-line cue).
    const requests =
      input.skill?.answerFormat === "full_only"
        ? allRequests.filter(([, part]) => part === "full")
        : allRequests;
    for (const [rid, part] of requests) {
      flight.requestIds.push(rid);
      this.byRequest.set(rid, { id, part });
    }
    flight.view = { ...flight.view, status: "streaming" };
    this.onUpdate({ ...flight.view });
    const results = await Promise.all(
      requests.map(([rid, , req]) => this.deps.llmStream(rid, plan.provider, req)),
    );
    const failed = results.find((r) => r.status === "error");
    if (failed?.status === "error") return this.fail(flight, failed.error.message);
    return flight.view;
  }

  /** Stops an answer's streams (user dismissed it, or a newer question needs the slot). */
  cancel(id: string) {
    const f = this.inFlight.get(id);
    if (!f) return;
    this.inFlight.delete(id);
    f.requestIds.forEach((r) => void this.deps.cancelStream(r));
    f.view = { ...f.view, status: "cancelled" };
    this.onUpdate({ ...f.view });
  }

  private onToken(requestId: string, delta: string) {
    const ref = this.byRequest.get(requestId);
    const f = ref && this.inFlight.get(ref.id);
    if (!ref || !f) return;
    if (ref.part === "cue") {
      if (!f.sawCue) {
        f.sawCue = true;
        void this.deps.recordLatency("question_to_cue", Math.max(0, this.now() - f.endedAt));
      }
      f.cueTokens += delta;
      f.view = { ...f.view, cue: f.cueTokens.trim() };
    } else {
      if (!f.sawBody) {
        f.sawBody = true;
        void this.deps.recordLatency("question_to_answer", Math.max(0, this.now() - f.endedAt));
      }
      f.bodyTokens += delta;
      f.view = { ...f.view, body: f.bodyTokens };
    }
    this.onUpdate({ ...f.view });
  }

  private onDone(
    requestId: string,
    usage: { inputTokens: number; outputTokens: number },
    finish: string,
  ) {
    const ref = this.byRequest.get(requestId);
    const f = ref && this.inFlight.get(ref.id);
    if (!ref || !f) return;
    this.byRequest.delete(requestId);
    if (finish === "refusal") {
      const note = "The model declined to answer this one.";
      if (ref.part === "cue") f.view = { ...f.view, cue: note };
      else f.view = { ...f.view, body: note };
    }
    if (ref.part === "cue") f.cueDone = true;
    else {
      f.bodyDone = true;
      f.usage = usage;
    }
    if (f.bodyDone && (f.cueDone || !f.requestIds.includes(`${f.view.id}:cue`)))
      void this.finish(f);
    else this.onUpdate({ ...f.view });
  }

  private onError(requestId: string, error: SidekickError) {
    const ref = this.byRequest.get(requestId);
    const f = ref && this.inFlight.get(ref.id);
    if (!ref || !f) return;
    this.byRequest.delete(requestId);
    if (error.code === "CANCELLED") return;
    if (ref.part === "cue") {
      f.cueDone = true; // A failed cue doesn't sink the full answer.
      if (f.bodyDone) void this.finish(f);
      return;
    }
    void this.fail(f, error.message);
  }

  private async finish(f: InFlight) {
    this.inFlight.delete(f.view.id);
    const cited = citedSources(f.bodyTokens, f.sources);
    f.view = { ...f.view, status: "done", sources: cited, cue: f.view.cue || "" };
    this.onUpdate({ ...f.view });
    const saved = await this.deps.saveAnswer({
      sessionId: f.input.sessionId,
      question: f.input.label ?? f.input.text,
      source: f.input.source,
      directed: f.input.directed,
      segmentId: f.input.segmentId ?? null,
      cue: f.view.cue || null,
      bodyMd: f.bodyTokens,
      sources: cited.map((s) => ({ type: s.type, ref: s.ref, label: s.label })),
      provider: f.input.models.provider,
      model: f.input.models.full,
      latencyMs: Math.max(0, this.now() - f.started),
      inputTokens: f.usage?.inputTokens ?? null,
      outputTokens: f.usage?.outputTokens ?? null,
    });
    if (saved.status === "ok") {
      f.view = { ...f.view, answerId: saved.data };
      this.onUpdate({ ...f.view });
    }
  }

  private fail(f: InFlight, message: string): AnswerView {
    this.inFlight.delete(f.view.id);
    f.requestIds.forEach((r) => {
      this.byRequest.delete(r);
      void this.deps.cancelStream(r);
    });
    f.view = { ...f.view, status: "error", error: message };
    this.onUpdate({ ...f.view });
    // Error messages are Sidekick's own wording, never content.
    log.warn(`answer failed: ${message}`, { requestId: f.view.id, sessionId: f.input.sessionId });
    return f.view;
  }
}
