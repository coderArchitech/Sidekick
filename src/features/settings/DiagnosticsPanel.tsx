import { useCallback, useEffect, useState } from "react";
import { chooseLogExportPath, commands, type Diagnostics, type Stage } from "@/ipc";

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
  const [exported, setExported] = useState<{ text: string; error: boolean } | null>(null);

  async function exportLogs() {
    setExported(null);
    const path = await chooseLogExportPath();
    if (!path) return;
    const r = await commands.exportLogs(path);
    setExported(
      r.status === "ok"
        ? {
            text: `Saved the log bundle (${r.data} log ${r.data === 1 ? "file" : "files"} and diagnostics).`,
            error: false,
          }
        : { text: r.error.message, error: true },
    );
  }

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
    <section aria-labelledby="diagnostics-heading" className="card p-4">
      <div className="flex items-center justify-between">
        <h2 id="diagnostics-heading" className="heading text-xl">
          Diagnostics
        </h2>
        <div className="flex gap-2">
          <button type="button" className="btn btn-secondary" onClick={() => void exportLogs()}>
            Export logs…
          </button>
          <button type="button" className="btn btn-secondary" onClick={refresh}>
            Refresh
          </button>
        </div>
      </div>
      {exported && (
        <p
          role={exported.error ? "alert" : "status"}
          className={`text-sm ${exported.error ? "text-danger" : "text-success"}`}
        >
          {exported.text}
        </p>
      )}
      <p className="mb-3 text-sm text-fg-muted">
        Measured on this computer over the last 15 minutes. Nothing here is sent anywhere. The log
        bundle holds logs (no keys, transcripts or answers) and these numbers, for a bug report.
      </p>
      {data && (
        <>
          <p className="text-sm text-fg-2">
            Sidekick {data.appVersion} on {data.os}. Screen-share protection:{" "}
            {data.protection.reason}
          </p>
          <table className="mt-3 w-full text-left text-sm">
            <caption className="sr-only">Latency by stage</caption>
            <thead className="text-fg-muted">
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
                <tr key={s.stage} className="border-t border-line">
                  <th scope="row" className="py-1 font-normal">
                    {STAGE_LABELS[s.stage]}
                  </th>
                  <td>{ms(s.p50Ms)}</td>
                  <td>{ms(s.p95Ms)}</td>
                  <td className="text-fg-muted">
                    {s.targetP50Ms} / {s.targetP95Ms} ms
                  </td>
                  <td>
                    {s.healthy === null ? (
                      <span className="text-fg-muted">No data</span>
                    ) : s.healthy ? (
                      <span className="text-success">Healthy</span>
                    ) : (
                      <span className="text-warning">Slow</span>
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
