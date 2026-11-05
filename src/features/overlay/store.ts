import { create } from "zustand";

export const TABS = ["answers", "transcript", "notes", "chat"] as const;
export type Tab = (typeof TABS)[number];

export const MIN_OPACITY = 0.2;
export const MAX_OPACITY = 1;

interface OverlayState {
  tab: Tab;
  opacity: number;
  setTab: (tab: Tab) => void;
  setOpacity: (opacity: number) => void;
}

const clamp = (value: number) => Math.min(MAX_OPACITY, Math.max(MIN_OPACITY, value));

export const useOverlayStore = create<OverlayState>((set) => ({
  tab: "answers",
  opacity: 0.9,
  setTab: (tab) => set({ tab }),
  setOpacity: (opacity) => set({ opacity: clamp(opacity) }),
}));
