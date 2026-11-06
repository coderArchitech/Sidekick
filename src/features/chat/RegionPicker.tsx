import { useEffect, useRef, useState, type PointerEvent } from "react";
import { commands, type Region } from "@/ipc";

const MIN_SIZE = 0.02;

/**
 * Pick an area on a small preview of the screen (PRD SCR-1). The preview is
 * never stored; only the chosen area is captured afresh at full resolution.
 */
export function RegionPicker({
  onPick,
  onCancel,
}: {
  onPick: (r: Region) => void;
  onCancel: () => void;
}) {
  const [src, setSrc] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [region, setRegion] = useState<Region | null>(null);
  const start = useRef<{ x: number; y: number } | null>(null);
  const box = useRef<HTMLDivElement>(null);

  useEffect(() => {
    void commands.previewScreen().then((r) => {
      if (r.status === "ok") setSrc(r.data);
      else setError(r.error.message);
    });
  }, []);

  function point(e: PointerEvent<HTMLDivElement>) {
    const rect = box.current?.getBoundingClientRect();
    if (!rect || rect.width === 0 || rect.height === 0) return { x: 0, y: 0 };
    const clamp = (v: number) => Math.min(1, Math.max(0, v));
    return {
      x: clamp((e.clientX - rect.left) / rect.width),
      y: clamp((e.clientY - rect.top) / rect.height),
    };
  }

  function move(e: PointerEvent<HTMLDivElement>) {
    if (!start.current) return;
    const p = point(e);
    const s = start.current;
    setRegion({
      x: Math.min(s.x, p.x),
      y: Math.min(s.y, p.y),
      width: Math.abs(p.x - s.x),
      height: Math.abs(p.y - s.y),
    });
  }

  const valid = !!region && region.width >= MIN_SIZE && region.height >= MIN_SIZE;

  return (
    <div
      role="dialog"
      aria-label="Select an area"
      className="absolute bottom-full left-0 z-10 mb-1 w-80 rounded-md border border-line bg-surface p-2 text-xs shadow-lg"
    >
      <p className="mb-1 text-fg-2">Drag over the area to capture.</p>
      {error && <p className="text-danger">{error}</p>}
      {!src && !error && <p className="text-fg-muted">Loading a preview…</p>}
      {src && (
        <div
          ref={box}
          data-testid="region-surface"
          className="relative cursor-crosshair touch-none select-none"
          onPointerDown={(e) => {
            start.current = point(e);
            setRegion(null);
            e.currentTarget.setPointerCapture?.(e.pointerId);
          }}
          onPointerMove={move}
          onPointerUp={(e) => {
            move(e);
            start.current = null;
          }}
        >
          <img src={src} alt="Your screen" draggable={false} className="block w-full rounded" />
          {region && (
            <div
              aria-hidden
              className="absolute border-2 border-accent bg-accent/20"
              style={{
                left: `${region.x * 100}%`,
                top: `${region.y * 100}%`,
                width: `${region.width * 100}%`,
                height: `${region.height * 100}%`,
              }}
            />
          )}
        </div>
      )}
      <div className="mt-2 flex justify-end gap-2">
        <button type="button" className="btn btn-ghost btn-sm" onClick={onCancel}>
          Cancel
        </button>
        <button
          type="button"
          disabled={!valid}
          className="btn btn-primary btn-sm"
          onClick={() => region && onPick(region)}
        >
          Capture area
        </button>
      </div>
    </div>
  );
}
