import { useCallback, useEffect, useState } from "react";
import { commands, type Diagnostics, type Stage } from "@/ipc";

const STAGE_LABELS: Record<Stage, string> = {
  transcript_lag: "Transcript lag",
  question_to_cue: "Question to cue",
  question_to_answer: "Question to full answer",
  llm_first_token: "LLM first token",
  retrieval: "Context retrieval",
};

const ms = (v: number | null) => (v === null ? "–" : `${v} ms`);

/** Settings → Diagnostics (tech doc 10.1). Local numbers only; nothing is sent anywhere. */
export function DiagnosticsPanel() {
  const [data, setData] = useState<Diagnostics | null>(null);
  const [version, setVersion] = useState(0);
  const refresh = useCallback(() => setVersion((v) => v + 1), []);

  useEffect(() => {
    let cancelled = false;
    void commands.getDiagnostics().then((d) => {
      if (!cancelled) setData(d);
    });
    return () => {
      cancelled = true;
    };
  }, [version]);

  return (
    <section
      aria-labelledby="diagnostics-heading"
      className="rounded-lg border border-slate-700 p-4"
    >
      <div className="flex items-center justify-between">
        <h2 id="diagnostics-heading" className="text-base font-semibold">
          Diagnostics
        </h2>
        <button
          type="button"
          className="rounded-md border border-slate-600 px-2.5 py-1 text-sm hover:bg-slate-700"
          onClick={refresh}
        >
          Refresh
        </button>
      </div>
      <p className="mb-3 text-sm text-slate-400">
        Measured on this computer over the last 15 minutes. Nothing here is sent anywhere.
      </p>
      {data && (
        <>
          <p className="text-sm text-slate-300">
            Sidekick {data.appVersion} on {data.os}. Screen-share protection:{" "}
            {data.protection.reason}
          </p>
          <table className="mt-3 w-full text-left text-sm">
            <caption className="sr-only">Latency by stage</caption>
            <thead className="text-slate-400">
              <tr>
                <th scope="col" className="py-1 font-normal">
                  Stage
                </th>
                <th scope="col" className="py-1 font-normal">
                  p50
                </th>
                <th scope="col" className="py-1 font-normal">
                  p95
                </th>
                <th scope="col" className="py-1 font-normal">
                  Target (p50 / p95)
                </th>
                <th scope="col" className="py-1 font-normal">
                  Status
                </th>
              </tr>
            </thead>
            <tbody>
              {data.stages.map((s) => (
                <tr key={s.stage} className="border-t border-slate-800">
                  <th scope="row" className="py-1 font-normal">
                    {STAGE_LABELS[s.stage]}
                  </th>
                  <td>{ms(s.p50Ms)}</td>
                  <td>{ms(s.p95Ms)}</td>
                  <td className="text-slate-400">
                    {s.targetP50Ms} / {s.targetP95Ms} ms
                  </td>
                  <td>
                    {s.healthy === null ? (
                      <span className="text-slate-400">No data</span>
                    ) : s.healthy ? (
                      <span className="text-emerald-300">Healthy</span>
                    ) : (
                      <span className="text-amber-300">Slow</span>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </section>
  );
}
