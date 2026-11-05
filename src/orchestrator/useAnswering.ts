import { useEffect } from "react";
import { answerEvents, commands, type Session } from "@/ipc";
import { createBatcher } from "@/lib/batcher";
import { AnswerRouter, type AnswerView } from "./answerer";
import { useAnswersStore } from "./answersStore";
import { modelPlan, type ModelPlan } from "./models";
import { useOrchestratorStore } from "./store";

/** The router for the current overlay, so Chat (T-023) can ask through it too. */
let current: { router: AnswerRouter; sessionId: string; models: () => ModelPlan } | null = null;

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

  useEffect(() => {
    if (!sessionId) return;
    let models: ModelPlan = modelPlan({ provider: "anthropic", model: null });
    void commands.getSettings().then((r) => {
      if (r.status === "ok") models = modelPlan(r.data.llm);
    });
    const { upsert, clear } = useAnswersStore.getState();
    clear();
    const batcher = createBatcher<AnswerView>(50, (views) => {
      // Only the latest state of each answer matters.
      const latest = new Map(views.map((v) => [v.id, v]));
      latest.forEach(upsert);
    });
    const router = new AnswerRouter({ ...commands, events: answerEvents }, (v) =>
      v.status === "streaming" ? batcher.push(v) : (batcher.flushNow(), upsert(v)),
    );
    current = { router, sessionId, models: () => models };
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
