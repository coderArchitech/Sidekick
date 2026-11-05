import ReactMarkdown from "react-markdown";
import { CodeBlock } from "./CodeBlock";

/**
 * Answer Markdown. Raw HTML in model output is never rendered (tech doc 11.5):
 * react-markdown escapes it unless a raw-HTML plugin is added, and links open
 * nothing inside the app.
 */
export function Markdown({ text }: { text: string }) {
  return (
    <ReactMarkdown
      skipHtml
      components={{
        pre: ({ children }) => <>{children}</>,
        code: ({ className, children }) => {
          const lang = /language-([\w+-]+)/.exec(className ?? "")?.[1];
          const code = String(children).replace(/\n$/, "");
          if (lang || code.includes("\n")) return <CodeBlock code={code} lang={lang ?? ""} />;
          return <code className="rounded bg-slate-800 px-1 text-[0.9em]">{children}</code>;
        },
        a: ({ children, href }) => (
          <span className="underline decoration-dotted" title={href}>
            {children}
          </span>
        ),
        ul: ({ children }) => <ul className="ml-4 list-disc">{children}</ul>,
        ol: ({ children }) => <ol className="ml-4 list-decimal">{children}</ol>,
        p: ({ children }) => <p className="my-1">{children}</p>,
      }}
    >
      {text}
    </ReactMarkdown>
  );
}
