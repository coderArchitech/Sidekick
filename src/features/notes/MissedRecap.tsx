import { useState } from "react";
import { Markdown } from "@/features/answers";
import { useSessionStore } from "@/features/session";
import { answerEvents, commands } from "@/ipc";
import { Select } from "@/lib/Select";
import { modelPlan, recap, sessionPlan, RECAP_MINUTES, type RecapMinutes } from "@/orchestrator";

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "done"; minutes: number; text: string }
  | { kind: "error"; message: string };

/** "What did I miss?" (PRD MEM-3): a one-click recap of the last few minutes. */
export function MissedRecap({ sessionId }: { sessionId: string }) {
  const [minutes, setMinutes] = useState<RecapMinutes>(5);
  const [state, setState] = useState<State>({ kind: "idle" });

  const run = async () => {
    setState({ kind: "loading" });
    try {
      const s = await commands.getSettings();
      const plan =
        s.status === "ok"
          ? sessionPlan(s.data.llm, useSessionStore.getState().session)
          : modelPlan({ provider: "anthropic", model: null });
      const text = await recap({ ...commands, events: answerEvents }, sessionId, minutes, plan);
      setState({ kind: "done", minutes, text });
    } catch (e) {
      setState({ kind: "error", message: e instanceof Error ? e.message : String(e) });
    }
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center gap-2">
        <button
          type="button"
          className="btn btn-secondary btn-sm"
          disabled={state.kind === "loading"}
          onClick={() => void run()}
        >
          What did I miss?
        </button>
        <div className="flex items-center gap-1 text-xs text-fg-muted">
          <span>Last</span>
          <Select
            aria-label="Recap period"
            small
            value={String(minutes)}
            onChange={(v) => setMinutes(Number(v) as RecapMinutes)}
            options={RECAP_MINUTES.map((m) => ({ value: String(m), label: `${m} min` }))}
          />
        </div>
      </div>
      {state.kind === "loading" && (
        <p role="status" className="text-xs text-fg-muted">
          Catching you up…
        </p>
      )}
      {state.kind === "error" && (
        <p role="alert" className="text-xs text-warning">
          Couldn't write the recap: {state.message}
        </p>
      )}
      {state.kind === "done" && (
        <section aria-label="What you missed" className="card relative p-2.5 text-sm text-fg-2">
          <div className="mb-1 flex items-center">
            <h2 className="text-xs font-semibold text-fg">Last {state.minutes} minutes</h2>
            <button
              type="button"
              className="btn btn-ghost btn-sm ml-auto"
              onClick={() => setState({ kind: "idle" })}
            >
              Dismiss
            </button>
          </div>
          <Markdown text={state.text} />
        </section>
      )}
    </div>
  );
}
