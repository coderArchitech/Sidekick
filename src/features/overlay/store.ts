import { create } from "zustand";
import { commands, type OverlayPatch } from "@/ipc";

export const TABS = ["answers", "transcript", "notes", "chat"] as const;
export type Tab = (typeof TABS)[number];

export const MIN_OPACITY = 0.2;
export const MAX_OPACITY = 1;
export const MIN_FONT = 12;
export const MAX_FONT = 20;

interface OverlayState {
  tab: Tab;
  /** 0.2 to 1. */
  opacity: number;
  /** Text size in px (PRD UI-3). */
  fontSize: number;
  /** Compact mode hides the session bar and documents (PRD UI-3). */
  compact: boolean;
  setTab: (tab: Tab) => void;
  setOpacity: (opacity: number) => void;
  setFontSize: (px: number) => void;
  setCompact: (compact: boolean) => void;
  /** Loads the saved appearance (PRD UI-1, UI-3). */
  load: () => Promise<void>;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

// Opacity changes continuously while dragging; save once it settles.
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let pending: OverlayPatch = {};
function save(patch: OverlayPatch, delayMs = 300) {
  pending = { ...pending, ...patch };
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    const p = pending;
    pending = {};
    void commands.setOverlay(p);
  }, delayMs);
}

export const useOverlayStore = create<OverlayState>((set) => ({
  tab: "answers",
  opacity: 0.8,
  fontSize: 14,
  compact: false,
  setTab: (tab) => set({ tab }),
  setOpacity: (opacity) => {
    const v = clamp(opacity, MIN_OPACITY, MAX_OPACITY);
    set({ opacity: v });
    save({ opacity: Math.round(v * 100) });
  },
  setFontSize: (px) => {
    const v = clamp(Math.round(px), MIN_FONT, MAX_FONT);
    set({ fontSize: v });
    save({ fontSize: v }, 0);
  },
  setCompact: (compact) => {
    set({ compact });
    save({ compact }, 0);
  },
  load: async () => {
    const r = await commands.getSettings();
    if (r.status !== "ok") return;
    const o = r.data.overlay;
    set({ opacity: o.opacity / 100, fontSize: o.fontSize, compact: o.compact });
  },
}));
