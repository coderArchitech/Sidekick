import { useEffect, useState } from "react";
import { commands, type WindowInfo } from "@/ipc";
import { useChatStore } from "@/orchestrator";
import { RegionPicker } from "./RegionPicker";
import { QuickActions, useThumb } from "./ScreenshotStrip";

type Step = "closed" | "menu" | "window" | "region";

/** Screenshot button for capture-and-ask (PRD SCR-1, SCR-4): screen, a window or a region. */
export function CaptureMenu({ sessionId, disabled }: { sessionId: string; disabled: boolean }) {
  const [step, setStep] = useState<Step>("closed");
  const [windows, setWindows] = useState<WindowInfo[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const capture = useChatStore((s) => s.capture);
  const capturing = useChatStore((s) => s.capturing);

  useEffect(() => {
    if (step !== "window") return;
    void commands.listWindows().then((r) => {
      if (r.status === "ok") setWindows(r.data);
      else setError(r.error.message);
    });
  }, [step]);

  async function take(request: Parameters<typeof capture>[1]) {
    setStep("closed");
    await capture(sessionId, request);
  }

  return (
    <div className="relative">
      <button
        type="button"
        aria-haspopup="menu"
        aria-expanded={step !== "closed"}
        disabled={disabled || capturing}
        onClick={() => {
          setError(null);
          setStep(step === "closed" ? "menu" : "closed");
        }}
        className="btn btn-secondary"
      >
        {capturing ? "Capturing…" : "Screenshot"}
      </button>

      {step === "menu" && (
        <div
          role="menu"
          aria-label="Capture"
          className="absolute bottom-full left-0 z-10 mb-1 w-40 rounded-md border border-line bg-popover p-1 text-sm shadow-lg"
        >
          <MenuItem onClick={() => void take({ mode: "screen" })}>Full screen</MenuItem>
          <MenuItem
            onClick={() => {
              setWindows(null);
              setStep("window");
            }}
          >
            A window…
          </MenuItem>
          <MenuItem onClick={() => setStep("region")}>An area…</MenuItem>
        </div>
      )}

      {step === "window" && (
        <div
          role="dialog"
          aria-label="Choose a window"
          className="absolute bottom-full left-0 z-10 mb-1 max-h-64 w-64 overflow-y-auto rounded-md border border-line bg-popover p-1 text-sm shadow-lg"
        >
          {error && <p className="p-1 text-danger">{error}</p>}
          {!windows && !error && <p className="p-1 text-fg-muted">Finding windows…</p>}
          {windows?.length === 0 && <p className="p-1 text-fg-muted">No other windows are open.</p>}
          {windows?.map((w) => (
            <button
              key={w.id}
              type="button"
              className="block w-full truncate rounded px-2 py-1 text-left hover:bg-surface-2"
              onClick={() => void take({ mode: "window", windowId: w.id })}
            >
              <span className="text-fg">{w.appName || "Window"}</span>
              {w.title && <span className="text-fg-muted"> — {w.title}</span>}
            </button>
          ))}
          <button
            type="button"
            className="mt-1 w-full rounded px-2 py-1 text-left text-fg-muted hover:bg-surface-2"
            onClick={() => setStep("closed")}
          >
            Cancel
          </button>
        </div>
      )}

      {step === "region" && (
        <RegionPicker
          onCancel={() => setStep("closed")}
          onPick={(region) => void take({ mode: "region", region })}
        />
      )}
    </div>
  );
}

function MenuItem({ onClick, children }: { onClick: () => void; children: string }) {
  return (
    <button
      type="button"
      role="menuitem"
      className="block w-full rounded px-2 py-1 text-left text-fg hover:bg-surface-2"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** The screenshot waiting for the next question, with a thumbnail and quick actions. */
export function PendingScreenshot({ disabled = false }: { disabled?: boolean }) {
  const pending = useChatStore((s) => s.pending);
  const clear = useChatStore((s) => s.clearPending);
  const src = useThumb(pending?.id ?? null);

  if (!pending) return null;
  return (
    <div className="space-y-1.5 rounded-md border border-line p-1.5">
      <div
        role="status"
        aria-label="Screenshot attached"
        className="flex items-center gap-2 text-xs text-fg-2"
      >
        {src && <img src={src} alt="Screenshot preview" className="h-10 rounded object-cover" />}
        <span className="flex-1">
          Screenshot attached ({pending.width}×{pending.height}). Ask about it below.
        </span>
        <button
          type="button"
          aria-label="Remove screenshot"
          className="rounded px-1.5 hover:bg-surface-2"
          onClick={clear}
        >
          ✕
        </button>
      </div>
      <QuickActions disabled={disabled} />
    </div>
  );
}
