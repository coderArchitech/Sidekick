import { useEffect, useState } from "react";
import { commands, type WindowInfo } from "@/ipc";
import { useChatStore } from "@/orchestrator";
import { RegionPicker } from "./RegionPicker";

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
        className="rounded-md border border-slate-700 px-2 py-1.5 text-sm text-slate-200 hover:bg-slate-800 disabled:opacity-40"
      >
        {capturing ? "Capturing…" : "Screenshot"}
      </button>

      {step === "menu" && (
        <div
          role="menu"
          aria-label="Capture"
          className="absolute bottom-full left-0 z-10 mb-1 w-40 rounded-md border border-slate-700 bg-slate-900 p-1 text-sm shadow-lg"
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
          className="absolute bottom-full left-0 z-10 mb-1 max-h-64 w-64 overflow-y-auto rounded-md border border-slate-700 bg-slate-900 p-1 text-sm shadow-lg"
        >
          {error && <p className="p-1 text-red-300">{error}</p>}
          {!windows && !error && <p className="p-1 text-slate-400">Finding windows…</p>}
          {windows?.length === 0 && (
            <p className="p-1 text-slate-400">No other windows are open.</p>
          )}
          {windows?.map((w) => (
            <button
              key={w.id}
              type="button"
              className="block w-full truncate rounded px-2 py-1 text-left hover:bg-slate-800"
              onClick={() => void take({ mode: "window", windowId: w.id })}
            >
              <span className="text-slate-100">{w.appName || "Window"}</span>
              {w.title && <span className="text-slate-400"> — {w.title}</span>}
            </button>
          ))}
          <button
            type="button"
            className="mt-1 w-full rounded px-2 py-1 text-left text-slate-400 hover:bg-slate-800"
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
      className="block w-full rounded px-2 py-1 text-left text-slate-100 hover:bg-slate-800"
      onClick={onClick}
    >
      {children}
    </button>
  );
}

/** The screenshot waiting for the next question, with a thumbnail. */
export function PendingScreenshot() {
  const pending = useChatStore((s) => s.pending);
  const clear = useChatStore((s) => s.clearPending);
  const [thumb, setThumb] = useState<{ id: string; src: string } | null>(null);
  const src = pending && thumb?.id === pending.id ? thumb.src : null;

  useEffect(() => {
    if (!pending) return;
    let live = true;
    void commands.screenshotImage(pending.id).then((r) => {
      if (live && r.status === "ok") setThumb({ id: pending.id, src: r.data });
    });
    return () => {
      live = false;
    };
  }, [pending]);

  if (!pending) return null;
  return (
    <div
      role="status"
      aria-label="Screenshot attached"
      className="flex items-center gap-2 rounded-md border border-slate-700 p-1 text-xs text-slate-300"
    >
      {src && <img src={src} alt="Screenshot preview" className="h-10 rounded object-cover" />}
      <span className="flex-1">
        Screenshot attached ({pending.width}×{pending.height}). Ask about it below.
      </span>
      <button
        type="button"
        aria-label="Remove screenshot"
        className="rounded px-1.5 hover:bg-slate-800"
        onClick={clear}
      >
        ✕
      </button>
    </div>
  );
}
