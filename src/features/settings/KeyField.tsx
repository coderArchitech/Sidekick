import { useId, useState } from "react";
import { commands, type KeyStatus } from "@/ipc";
import { keyErrorText } from "./errors";

interface Props {
  provider: string;
  providerName: string;
  status: KeyStatus;
  /** Called after a key is saved or removed, so the parent can refresh. */
  onChanged: () => void;
}

type Message = { kind: "ok" | "error"; text: string } | null;

/**
 * Masked API key input with show/hide and paste, Test connection and Remove
 * (PRD KEY-4, KEY-5, KEY-7). The typed key lives only in this component's
 * state until it is sent to Rust, and is cleared right after.
 */
export function KeyField({ provider, providerName, status, onChanged }: Props) {
  const inputId = useId();
  const [value, setValue] = useState("");
  const [visible, setVisible] = useState(false);
  const [replacing, setReplacing] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<Message>(null);

  const editing = !status.saved || replacing;

  async function save() {
    setBusy(true);
    setMessage(null);
    const result = await commands.setApiKey(provider, value);
    setBusy(false);
    if (result.status === "ok") {
      setValue("");
      setVisible(false);
      setReplacing(false);
      setMessage({
        kind: "ok",
        text: result.data.verified
          ? "Key works and is saved."
          : "Key saved. Sidekick can't test this provider yet.",
      });
      onChanged();
    } else {
      setMessage({ kind: "error", text: keyErrorText(result.error, providerName) });
    }
  }

  async function test() {
    setBusy(true);
    setMessage(null);
    const result = await commands.testApiKey(provider);
    setBusy(false);
    setMessage(
      result.status === "ok"
        ? {
            kind: "ok",
            text: result.data.verified
              ? "Connection works."
              : "Key is saved but can't be tested yet.",
          }
        : { kind: "error", text: keyErrorText(result.error, providerName) },
    );
  }

  async function remove() {
    setBusy(true);
    const result = await commands.deleteApiKey(provider);
    setBusy(false);
    if (result.status === "ok") {
      setMessage({ kind: "ok", text: "Key removed." });
      onChanged();
    } else {
      setMessage({ kind: "error", text: keyErrorText(result.error, providerName) });
    }
  }

  async function paste() {
    try {
      const text = await navigator.clipboard.readText();
      setValue(text.trim());
    } catch {
      setMessage({
        kind: "error",
        text: "Couldn't read the clipboard. Paste with Ctrl/Cmd+V instead.",
      });
    }
  }

  const button = "btn btn-secondary";

  return (
    <div className="mt-3 space-y-2">
      {editing ? (
        <form
          className="space-y-2"
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <label htmlFor={inputId} className="block text-sm text-fg-2">
            {providerName} API key
          </label>
          <div className="flex flex-wrap gap-2">
            <input
              id={inputId}
              type={visible ? "text" : "password"}
              autoComplete="off"
              spellCheck={false}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              className="min-w-0 flex-1 field font-mono text-sm"
            />
            <button
              type="button"
              className={button}
              aria-pressed={visible}
              onClick={() => setVisible((v) => !v)}
            >
              {visible ? "Hide" : "Show"}
            </button>
            <button type="button" className={button} onClick={() => void paste()}>
              Paste
            </button>
          </div>
          <div className="flex gap-2">
            <button
              type="submit"
              className={`${button} bg-accent hover:bg-accent-hover`}
              disabled={busy || !value.trim()}
            >
              {busy ? "Testing…" : "Save and test"}
            </button>
            {replacing && (
              <button type="button" className={button} onClick={() => setReplacing(false)}>
                Cancel
              </button>
            )}
          </div>
        </form>
      ) : (
        <div className="flex flex-wrap items-center gap-2">
          <span className="text-sm text-fg-2">
            Saved key ending in <span className="font-mono">••••{status.last4}</span>
          </span>
          <button type="button" className={button} disabled={busy} onClick={() => void test()}>
            {busy ? "Testing…" : "Test connection"}
          </button>
          <button type="button" className={button} onClick={() => setReplacing(true)}>
            Replace
          </button>
          <button type="button" className={button} disabled={busy} onClick={() => void remove()}>
            Remove key
          </button>
        </div>
      )}
      {message && (
        <p
          role={message.kind === "error" ? "alert" : "status"}
          className={`text-sm ${message.kind === "error" ? "text-danger" : "text-success"}`}
        >
          {message.text}
        </p>
      )}
    </div>
  );
}
