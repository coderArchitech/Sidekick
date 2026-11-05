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
  openai: { full: "gpt-5", cue: "gpt-5-mini" },
  gemini: { full: "gemini-2.5-pro", cue: "gemini-2.5-flash" },
  // Ollama: "default" is the first installed model, for both (local, no small/large pair).
};

export function modelPlan(llm: ProviderChoice): ModelPlan {
  const d = DEFAULTS[llm.provider];
  const full = llm.model ?? d?.full ?? "default";
  return { provider: llm.provider, full, cue: d?.cue ?? full };
}
