import { useSessionStore } from "@/features/session";
import { useAnswersStore } from "@/orchestrator";
import { AnswerCard } from "./AnswerCard";

/** The Live Answers tab (PRD UI-7, QA-5): newest answer first. */
export function LiveAnswers() {
  const session = useSessionStore((s) => s.session);
  const views = useAnswersStore((s) => s.views);
  if (views.length === 0) {
    return (
      <p className="text-slate-400">
        {session?.status === "live"
          ? session.autoAnswer
            ? "Listening for questions…"
            : "Listening for questions that use your name…"
          : "Start a session and answers to detected questions appear here."}
      </p>
    );
  }
  return (
    <div className="space-y-2">
      {views.map((v) => (
        <AnswerCard key={v.id} view={v} />
      ))}
    </div>
  );
}
