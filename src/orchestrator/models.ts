import type { ProviderChoice } from "@/ipc";

export interface ModelPlan {
  provider: string;
  /** Full answers (tech doc 3.3). */
  full: string;
  /** One-line cues: small and fast (PRD MOD-3). */
  cue: string;
}

/** Defaults per provider; the user's model choice wins for full answers. */
const DEFAULTS: Record<string, { full: string; cue: string }> = {
  anthropic: { full: "claude-opus-5-5", cue: "claude-haiku-5-5" },
};

export function modelPlan(llm: ProviderChoice): ModelPlan {
  const d = DEFAULTS[llm.provider];
  const full = llm.model ?? d?.full ?? "default";
  return { provider: llm.provider, full, cue: d?.cue ?? full };
}
