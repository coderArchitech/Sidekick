import { useEffect } from "react";
import { useSessionStore } from "@/features/session";
import { bookmarkNow } from "@/features/transcript";
import { sessionEvents, type Action } from "@/ipc";
import { nextSkillId, useChatStore, useSkillsStore } from "@/orchestrator";
import { useOverlayStore } from "./store";

/**
 * Runs overlay actions for global shortcuts (PRD UI-6). Show/hide is handled in
 * Rust. Ask opens Chat and focuses its input; Screenshot captures the screen
 * and opens Chat to ask about it; Switch Skill cycles enabled Skills, then none.
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
    case "screenshot":
      // Capture-and-ask (SCR-4): full screen now, then type the question.
      if (store.session && store.session.status !== "ended") {
        useOverlayStore.getState().setTab("chat");
        await useChatStore.getState().capture(store.session.id, { mode: "screen" });
      }
      break;
    case "switch_skill": {
      if (!store.session || store.session.status === "ended") break;
      await useSkillsStore.getState().load();
      const next = nextSkillId(useSkillsStore.getState().skills, store.session.skillId);
      await store.setSkill(next);
      break;
    }
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
