import { useEffect, useState } from "react";
import { highlight } from "./highlight";

interface Props {
  code: string;
  lang: string;
}

/** A fenced code block with highlighting and a one-click copy button (PRD QA-7). */
export function CodeBlock({ code, lang }: Props) {
  const [html, setHtml] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void highlight(code, lang)
      .then((h) => {
        if (!cancelled) setHtml(h);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [code, lang]);

  return (
    <div className="relative my-1.5 rounded-md border border-slate-700 bg-slate-950 text-xs">
      <div className="flex items-center justify-between border-b border-slate-800 px-2 py-0.5 text-slate-400">
        <span>{lang || "code"}</span>
        <button
          type="button"
          className="rounded px-1.5 hover:bg-slate-800"
          aria-label={copied ? "Copied" : "Copy code"}
          onClick={() =>
            void navigator.clipboard.writeText(code).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            })
          }
        >
          {copied ? "Copied" : "Copy"}
        </button>
      </div>
      {html ? (
        // Shiki escapes the code; its output contains only spans with styles.
        <div
          className="overflow-x-auto p-2 [&_pre]:!bg-transparent"
          dangerouslySetInnerHTML={{ __html: html }}
        />
      ) : (
        <pre className="overflow-x-auto p-2">
          <code>{code}</code>
        </pre>
      )}
    </div>
  );
}
