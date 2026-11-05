import { useEffect } from "react";
import { useSessionStore } from "@/features/session";
import { bookmarkNow } from "@/features/transcript";
import { sessionEvents, type Action } from "@/ipc";
import { useChatStore } from "@/orchestrator";
import { useOverlayStore } from "./store";

/**
 * Runs overlay actions for global shortcuts (PRD UI-6). Show/hide is handled in
 * Rust. Ask opens Chat and focuses its input. Screenshot and Switch Skill hook
 * in when their features land (T-025, T-028).
 */
export function useHotkeyActions() {
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void sessionEvents
      .onHotkey((e) => void handle(e.action))
      .then((u) => (disposed ? u() : (unlisten = u)));
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}

export async function handle(action: Action): Promise<void> {
  const store = useSessionStore.getState();
  switch (action) {
    case "pause_resume":
      if (store.session?.status === "live") await store.pause();
      else if (store.session?.status === "paused") await store.resume();
      break;
    case "bookmark":
      await bookmarkNow(store.session);
      break;
    case "ask":
      useOverlayStore.getState().setTab("chat");
      useChatStore.getState().requestFocus();
      break;
    case "auto_answer":
      await store.toggleAuto();
      break;
    default:
      break;
  }
}
