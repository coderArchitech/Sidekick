import { useEffect, useRef } from "react";
import { commands, sessionEvents, type Session } from "@/ipc";
import { shouldAnswer } from "./detect";
import { QuestionDetector } from "./detector";
import { useOrchestratorStore } from "./store";

/**
 * Feeds the live transcript of `session` into question detection and queues
 * what it finds (QA-1 to QA-3). Auto-answer is read at decision time, so the
 * toggle applies immediately.
 */
export function useQuestionDetection(session: Session | null) {
  const sessionRef = useRef(session);
  useEffect(() => {
    sessionRef.current = session;
  });
  const sessionId = session?.status === "live" || session?.status === "paused" ? session.id : null;

  useEffect(() => {
    if (!sessionId) return;
    const { add, clear } = useOrchestratorStore.getState();
    clear();
    const detector = new QuestionDetector({
      names: [],
      onQuestion: (q) =>
        add({ ...q, answer: shouldAnswer(q, sessionRef.current?.autoAnswer ?? true) }),
    });
    void commands.getSettings().then((r) => {
      if (r.status === "ok")
        detector.setNames([r.data.profile.name, ...r.data.profile.nicknames].filter(Boolean));
    });
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void sessionEvents
      .onTranscriptSegment((e) => {
        if (e.sessionId === sessionId) detector.push(e);
      })
      .then((u) => (disposed ? u() : (unlisten = u)));
    return () => {
      disposed = true;
      unlisten?.();
      detector.dispose();
    };
  }, [sessionId]);
}
