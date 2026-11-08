import { useId, useState } from "react";

interface Props {
  vocabulary: string[];
  onSave: (vocabulary: string[]) => Promise<{ saved: string[] } | { error: string }>;
}

/** Most terms, as in Rust (`sidekick_settings::MAX_VOCABULARY`). */
const MAX_TERMS = 100;

/**
 * Settings → Custom vocabulary (PRD LIS-5): product names, acronyms and
 * people the speech-to-text should expect. Sent with your name and
 * nicknames to every provider when a session starts.
 */
export function VocabularySection({ vocabulary, onSave }: Props) {
  const [text, setText] = useState(vocabulary.join("\n"));
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const id = useId();
  const terms = text
    .split(/[\n,]/)
    .map((t) => t.trim())
    .filter(Boolean);

  async function save() {
    const r = await onSave(terms);
    if ("error" in r) setMessage({ ok: false, text: r.error });
    else {
      setText(r.saved.join("\n"));
      setMessage({
        ok: true,
        text: r.saved.length
          ? `Saved ${r.saved.length} term${r.saved.length === 1 ? "" : "s"}. They apply from the next session.`
          : "Vocabulary cleared.",
      });
    }
  }

  return (
    <section aria-labelledby="vocabulary-heading" className="card p-4">
      <h2 id="vocabulary-heading" className="text-base font-semibold">
        Custom vocabulary
      </h2>
      <p className="mb-3 text-sm text-fg-muted">
        Product names, acronyms and people speech-to-text should expect, one per line (or
        comma-separated). Up to {MAX_TERMS} terms of up to 50 characters.
      </p>
      <label htmlFor={id} className="sr-only">
        Vocabulary terms
      </label>
      <textarea
        id={id}
        rows={5}
        className="field w-full font-mono text-sm"
        placeholder={"Kubernetes\nSOC 2\nPriya Raman"}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setMessage(null);
        }}
      />
      <div className="mt-2 flex items-center gap-3">
        <button type="button" className="btn btn-secondary" onClick={() => void save()}>
          Save vocabulary
        </button>
        <span className="text-xs text-fg-muted tabular">
          {terms.length}/{MAX_TERMS} terms
        </span>
        {message && (
          <p
            role={message.ok ? "status" : "alert"}
            className={`text-sm ${message.ok ? "text-success" : "text-danger"}`}
          >
            {message.text}
          </p>
        )}
      </div>
    </section>
  );
}
