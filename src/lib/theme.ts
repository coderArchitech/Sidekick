import { useEffect, useState } from "react";
import { commands, sessionEvents, type Theme } from "@/ipc";

export type ResolvedTheme = "dark" | "light" | "high-contrast";

/** The theme to show: System follows the OS light/dark preference (PRD UI-8). */
export function resolveTheme(theme: Theme, prefersLight: boolean): ResolvedTheme {
  if (theme === "system") return prefersLight ? "light" : "dark";
  return theme === "high_contrast" ? "high-contrast" : theme;
}

const query = () =>
  typeof window.matchMedia === "function"
    ? window.matchMedia("(prefers-color-scheme: light)")
    : null;

/** Sets `data-theme` on <html> (styles.css holds the tokens). */
export function applyTheme(theme: Theme): ResolvedTheme {
  const resolved = resolveTheme(theme, query()?.matches ?? false);
  document.documentElement.dataset.theme = resolved;
  return resolved;
}

/**
 * Keeps this window's theme in step with Settings: on load, whenever any
 * window saves settings, and when the OS switches light/dark under System.
 */
export function useAppTheme() {
  const [theme, setTheme] = useState<Theme>("dark");

  useEffect(() => {
    let live = true;
    void commands.getSettings().then((r) => {
      if (live && r.status === "ok") setTheme(r.data.theme);
    });
    let unlisten: (() => void) | undefined;
    void sessionEvents
      .onSettingsChanged((e) => setTheme(e.settings.theme))
      .then((u) => (live ? (unlisten = u) : u()));
    return () => {
      live = false;
      unlisten?.();
    };
  }, []);

  useEffect(() => {
    applyTheme(theme);
    const mq = query();
    if (theme !== "system" || !mq) return;
    const follow = () => applyTheme(theme);
    mq.addEventListener("change", follow);
    return () => mq.removeEventListener("change", follow);
  }, [theme]);
}
