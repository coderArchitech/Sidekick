/**
 * Code highlighting with Shiki (tech doc 3.4), loaded on first use with a small
 * language set and the JavaScript regex engine (no WASM), to keep startup and
 * the installer small. Shiki escapes the code, so its HTML is safe to insert.
 */
import type { HighlighterCore } from "shiki/core";

const LANGS = [
  "typescript",
  "javascript",
  "tsx",
  "python",
  "json",
  "bash",
  "rust",
  "go",
  "java",
  "sql",
  "yaml",
  "html",
  "css",
];
const ALIASES: Record<string, string> = {
  ts: "typescript",
  js: "javascript",
  py: "python",
  sh: "bash",
  shell: "bash",
  yml: "yaml",
  rs: "rust",
};

let loading: Promise<HighlighterCore> | null = null;

function highlighter(): Promise<HighlighterCore> {
  loading ??= (async () => {
    const [{ createHighlighterCore }, { createJavaScriptRegexEngine }, theme, ...langs] =
      await Promise.all([
        import("shiki/core"),
        import("shiki/engine/javascript"),
        import("shiki/themes/github-dark.mjs"),
        import("shiki/langs/typescript.mjs"),
        import("shiki/langs/javascript.mjs"),
        import("shiki/langs/tsx.mjs"),
        import("shiki/langs/python.mjs"),
        import("shiki/langs/json.mjs"),
        import("shiki/langs/bash.mjs"),
        import("shiki/langs/rust.mjs"),
        import("shiki/langs/go.mjs"),
        import("shiki/langs/java.mjs"),
        import("shiki/langs/sql.mjs"),
        import("shiki/langs/yaml.mjs"),
        import("shiki/langs/html.mjs"),
        import("shiki/langs/css.mjs"),
      ]);
    return createHighlighterCore({
      themes: [theme.default],
      langs: langs.map((l) => l.default),
      engine: createJavaScriptRegexEngine(),
    });
  })();
  return loading;
}

/** Highlighted HTML, or null for unknown languages (shown as plain text). */
export async function highlight(code: string, lang: string): Promise<string | null> {
  const name = ALIASES[lang] ?? lang;
  if (!LANGS.includes(name)) return null;
  const h = await highlighter();
  return h.codeToHtml(code, { lang: name, theme: "github-dark" });
}
