import { useState } from "react";
import { useSessionStore } from "@/features/session";
import { captureAndAnswer } from "@/orchestrator";
import { CaptureMenu } from "./CaptureMenu";

/**
 * Screenshot on the Live Answers tab: the capture is read as the question
 * (what's on screen is answered right there), so an interviewer's problem
 * statement or a shared slide can be answered without switching to Chat.
 */
export function LiveScreenshot() {
  const session = useSessionStore((s) => s.session);
  const [error, setError] = useState<string | null>(null);
  if (!session) return null;
  const ended = session.status === "ended";
  return (
    <div className="mb-2 flex items-center gap-2 text-xs">
      <CaptureMenu
        sessionId={session.id}
        disabled={ended}
        down
        onTake={async (request) => setError(await captureAndAnswer(request))}
      />
      <span className="text-fg-muted">
        {ended ? "The session has ended." : "Answers the question on your screen."}
      </span>
      {error && (
        <p role="alert" className="text-danger">
          {error}
        </p>
      )}
    </div>
  );
}
