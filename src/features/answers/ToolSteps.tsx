import type { AnswerView } from "@/orchestrator";

const NAMES: Record<string, string> = {
  calculator: "Calculator",
  doc_search: "Document search",
  web_search: "Web search",
};

/** What an agent Skill's tools did for this answer (PRD SKL-4). */
export function ToolSteps({ view }: { view: AnswerView }) {
  if (!view.tools) return null;
  if (view.tools.length === 0)
    return view.status === "thinking" ? (
      <p className="mt-1 text-xs text-fg-muted">Checking whether tools help…</p>
    ) : null;
  return (
    <ul aria-label="Tools used" className="mt-1.5 space-y-0.5 text-xs text-fg-muted">
      {view.tools.map((t, i) => (
        <li key={i} className="truncate" title={`${t.input} → ${t.output}`}>
          <span className="font-medium text-fg-2">{NAMES[t.tool] ?? t.tool}</span>: {t.input}{" "}
          {t.ok ? (
            t.tool === "calculator" ? (
              <span className="text-fg-2">= {t.output}</span>
            ) : null
          ) : (
            <span className="text-warning">({t.output})</span>
          )}
        </li>
      ))}
    </ul>
  );
}
