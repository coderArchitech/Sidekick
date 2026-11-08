import { useEffect, useRef } from "react";
import { answerEvents, commands, type QuestionSource, type Session } from "@/ipc";
import { createBatcher } from "@/lib/batcher";
import { AnswerRouter, type AnswerView } from "./answerer";
import { useAnswersStore } from "./answersStore";
import { chatHistory, useChatStore } from "./chatStore";
import { followUpInput, type FollowUpKind } from "./followUps";
import { screenshotAction, type ScreenshotActionKind } from "./screenshotActions";
import { modelPlan, type ModelPlan } from "./models";
import type { SkillPrompt } from "./prompt";
import { skillsFor, useSkillsStore } from "./skills";
import { useOrchestratorStore } from "./store";

/** The router for the current overlay, so Chat (T-023) can ask through it too. */
let current: {
  router: AnswerRouter;
  sessionId: string;
  models: () => ModelPlan;
  /** The active Skill for a question from `source` (PRD SKL-1). */
  skill: (source: QuestionSource) => SkillPrompt | null;
} | null = null;

export function activeRouter() {
  return current;
}

/**
 * Answers queued questions for `session` (tech doc 3.3): every question the
 * detector marked `answer` goes to the router. UI updates are batched every
 * 50 ms while tokens stream (tech doc 13.5).
 */
export function useAnswering(session: Session | null) {
  const sessionId = session && session.status !== "ended" ? session.id : null;
  // Read at question time, so switching Skills applies to the next question.
  const skillIds = useRef<(string | null)[]>([]);
  const activeSkill = session?.skillId ?? null;
  const extraSkills = (session?.extraSkillIds ?? []).join("\u0000");
  useEffect(() => {
    skillIds.current = [activeSkill, ...(extraSkills ? extraSkills.split("\u0000") : [])];
  }, [activeSkill, extraSkills]);

  useEffect(() => {
    if (!sessionId) return;
    let models: ModelPlan = modelPlan({ provider: "anthropic", model: null });
    void commands.getSettings().then((r) => {
      if (r.status === "ok") models = modelPlan(r.data.llm);
    });
    const { clear } = useAnswersStore.getState();
    clear();
    void useChatStore.getState().load(sessionId);
    void useSkillsStore.getState().load();
    const skill = (source: QuestionSource) =>
      skillsFor(useSkillsStore.getState().skills, skillIds.current, source);
    // Questions the user asked (Chat, capture-and-ask) go to the Chat tab;
    // detected ones to Live Answers.
    const upsert = (v: AnswerView) =>
      v.source !== "detected"
        ? useChatStore.getState().upsert(v)
        : useAnswersStore.getState().upsert(v);
    const batcher = createBatcher<AnswerView>(50, (views) => {
      // Only the latest state of each answer matters.
      const latest = new Map(views.map((v) => [v.id, v]));
      latest.forEach(upsert);
    });
    const router = new AnswerRouter({ ...commands, events: answerEvents }, (v) =>
      v.status === "streaming" ? batcher.push(v) : (batcher.flushNow(), upsert(v)),
    );
    current = { router, sessionId, models: () => models, skill };
    const seen = new Set<string>();
    const unsubscribe = useOrchestratorStore.subscribe((state) => {
      for (const q of state.questions) {
        const key = `${q.detectedAt}:${q.segmentIds.join(",")}`;
        if (seen.has(key) || !q.answer) continue;
        seen.add(key);
        void router.ask({
          sessionId,
          text: q.text,
          directed: q.directed,
          source: "detected",
          segmentId: q.segmentIds[q.segmentIds.length - 1] ?? null,
          // The question ended about one debounce before detection.
          endedAt: q.detectedAt - 600,
          models,
          skill: skill("detected"),
        });
      }
    });
    return () => {
      unsubscribe();
      router.dispose();
      batcher.dispose();
      if (current?.router === router) current = null;
    };
  }, [sessionId]);
}

/**
 * Asks a typed Chat question with the session context and recent chat turns
 * as history (PRD QA-8). Returns an error message when there's no session to
 * answer from.
 */
export async function askChat(text: string): Promise<string | null> {
  const q = text.trim();
  if (!q) return null;
  const active = current;
  if (!active) return "Start a session to ask questions.";
  // A question asked right after a capture is about that screenshot (SCR-4).
  const chat = useChatStore.getState();
  const screenshot = chat.pending;
  const aboutScreenshot = !!screenshot;
  chat.clearPending();
  await active.router.ask({
    sessionId: active.sessionId,
    text: q,
    directed: true,
    source: aboutScreenshot ? "screenshot" : "chat",
    skill: active.skill(aboutScreenshot ? "screenshot" : "chat"),
    endedAt: Date.now(),
    models: active.models(),
    history: chatHistory(useChatStore.getState().turns),
    // That exact screenshot, also when it was re-attached from the strip.
    screenshotId: screenshot?.id,
  });
  return null;
}

/**
 * Runs a quick action ("Explain", "Extract table", …) on the attached
 * screenshot (PRD SCR-5). The turn shows as "Extract table (screenshot)".
 */
export async function askScreenshotAction(kind: ScreenshotActionKind): Promise<string | null> {
  const active = current;
  if (!active) return "Start a session to ask questions.";
  const chat = useChatStore.getState();
  const screenshot = chat.pending;
  if (!screenshot) return "Take or pick a screenshot first.";
  chat.clearPending();
  const action = screenshotAction(kind);
  await active.router.ask({
    sessionId: active.sessionId,
    text: action.prompt,
    label: `${action.label} (screenshot)`,
    directed: true,
    source: "screenshot",
    skill: active.skill("screenshot"),
    endedAt: Date.now(),
    models: active.models(),
    screenshotId: screenshot.id,
  });
  return null;
}

/**
 * Asks a follow-up chip ("Shorter", "More detail", …) on a finished answer
 * (PRD QA-9). The new answer goes to Chat, with the original exchange as
 * history, and the overlay switches to Chat to show it.
 */
export async function askFollowUp(view: AnswerView, kind: FollowUpKind): Promise<string | null> {
  const active = current;
  if (!active) return "Start a session to ask questions.";
  const f = followUpInput(view, kind);
  useChatStore.getState().requestShow();
  await active.router.ask({
    sessionId: active.sessionId,
    text: f.text,
    label: f.label,
    query: f.query,
    directed: true,
    source: "chat",
    skill: active.skill("chat"),
    endedAt: Date.now(),
    models: active.models(),
    history: f.history,
  });
  return null;
}
