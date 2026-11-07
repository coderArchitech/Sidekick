import { useEffect, useRef, useState } from "react";
import { MAX_FONT, MAX_OPACITY, MIN_FONT, MIN_OPACITY, useOverlayStore } from "./store";

/** Display options in one menu, to keep the header calm (PRD UI-1, UI-3). */
export function ViewMenu() {
  const { opacity, fontSize, compact, setOpacity, setFontSize, setCompact } = useOverlayStore();
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (
        e instanceof KeyboardEvent ? e.key === "Escape" : !root.current?.contains(e.target as Node)
      )
        setOpen(false);
    };
    document.addEventListener("mousedown", close);
    document.addEventListener("keydown", close);
    return () => {
      document.removeEventListener("mousedown", close);
      document.removeEventListener("keydown", close);
    };
  }, [open]);

  return (
    <div ref={root} className="relative">
      <button
        type="button"
        className={`btn btn-ghost btn-sm ${open ? "bg-surface-3 text-fg" : ""}`}
        aria-haspopup="dialog"
        aria-expanded={open}
        onClick={() => setOpen((o) => !o)}
      >
        View
      </button>
      {open && (
        <div
          role="dialog"
          aria-label="View options"
          className="absolute right-0 z-20 mt-1 w-56 space-y-3 rounded-lg border border-line bg-popover p-3 text-xs text-fg-2 shadow-xl shadow-black/40"
        >
          <div className="flex items-center justify-between">
            <span>Text size</span>
            <div className="flex items-center gap-1" role="group" aria-label="Text size">
              <button
                type="button"
                className="btn btn-secondary btn-sm px-2"
                disabled={fontSize <= MIN_FONT}
                onClick={() => setFontSize(fontSize - 1)}
              >
                <span aria-hidden="true">A−</span>
                <span className="sr-only">Smaller text</span>
              </button>
              <span className="tabular w-8 text-center text-fg">{fontSize}px</span>
              <button
                type="button"
                className="btn btn-secondary btn-sm px-2"
                disabled={fontSize >= MAX_FONT}
                onClick={() => setFontSize(fontSize + 1)}
              >
                <span aria-hidden="true">A+</span>
                <span className="sr-only">Larger text</span>
              </button>
            </div>
          </div>
          <label className="flex items-center justify-between gap-2">
            <span>
              Background
              <span className="block text-fg-muted tabular">{Math.round(opacity * 100)}%</span>
            </span>
            <input
              type="range"
              className="w-28"
              aria-label="Background opacity"
              min={MIN_OPACITY}
              max={MAX_OPACITY}
              step={0.05}
              value={opacity}
              onChange={(e) => setOpacity(Number(e.target.value))}
            />
          </label>
          <label className="flex items-center justify-between gap-2">
            <span>
              Compact
              <span className="block text-fg-muted">Hide the session bar and documents</span>
            </span>
            <input
              type="checkbox"
              role="switch"
              aria-label="Compact"
              checked={compact}
              onChange={(e) => setCompact(e.target.checked)}
            />
          </label>
        </div>
      )}
    </div>
  );
}
