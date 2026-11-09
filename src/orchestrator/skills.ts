import { create } from "zustand";
import { commands, type QuestionSource, type SkillEntry, type Trigger } from "@/ipc";
import type { SkillPrompt } from "./prompt";

interface SkillsState {
  skills: SkillEntry[];
  load: () => Promise<void>;
}

/** Skills the overlay can use (read-only; editing is in Settings). */
export const useSkillsStore = create<SkillsState>((set) => ({
  skills: [],
  load: async () => {
    const r = await commands.listSkills();
    if (r.status === "ok") set({ skills: r.data });
  },
}));

const TRIGGER_FOR: Record<QuestionSource, Trigger> = {
  detected: "question_detected",
  chat: "chat",
  screenshot: "screenshot",
};

/**
 * The prompt part of the active Skill for a question from `source`, or null
 * when there's no Skill, it's off, or it doesn't apply to that source.
 */
export function skillFor(
  skills: SkillEntry[],
  skillId: string | null | undefined,
  source: QuestionSource,
): SkillPrompt | null {
  const s = skills.find((x) => x.id === skillId);
  if (!s?.enabled) return null;
  const triggers = s.triggers ?? ["question_detected", "chat", "screenshot"];
  if (!triggers.includes(TRIGGER_FOR[source])) return null;
  return {
    name: s.name,
    instructions: s.instructions,
    answerFormat: s.answerFormat ?? "cue_then_full",
    ...(s.tools?.length ? { tools: s.tools } : {}),
  };
}

/**
 * The combined prompt of the session's active Skills (PRD SKL-5): the main
 * one first, then the others, keeping those that apply to `source`. One-line
 * cues stay unless every applicable Skill is full-only; tools are pooled.
 */
export function skillsFor(
  skills: SkillEntry[],
  skillIds: (string | null | undefined)[],
  source: QuestionSource,
): SkillPrompt | null {
  const parts = [...new Set(skillIds)]
    .map((id) => skillFor(skills, id, source))
    .filter((p): p is SkillPrompt => p !== null);
  if (parts.length === 0) return null;
  if (parts.length === 1) return parts[0] ?? null;
  const tools = [...new Set(parts.flatMap((p) => p.tools ?? []))];
  return {
    name: parts.map((p) => p.name).join(" + "),
    instructions: parts.map((p) => `### ${p.name}\n${p.instructions.trim()}`).join("\n\n"),
    answerFormat: parts.every((p) => p.answerFormat === "full_only")
      ? "full_only"
      : "cue_then_full",
    ...(tools.length ? { tools } : {}),
  };
}

/** The Skill after `current` among enabled ones, then none (Switch Skill shortcut). */
export function nextSkillId(
  skills: SkillEntry[],
  current: string | null | undefined,
): string | null {
  const enabled = skills.filter((s) => s.enabled);
  if (enabled.length === 0) return null;
  const i = enabled.findIndex((s) => s.id === current);
  if (i === -1) return enabled[0]?.id ?? null;
  return enabled[i + 1]?.id ?? null;
}
