import type { AnswerEvents, LlmRequest, Result, SidekickError } from "@/ipc";

export interface CompleteDeps {
  llmStream(
    requestId: string,
    provider: string | null,
    request: LlmRequest,
  ): Promise<Result<null, SidekickError>>;
  events: AnswerEvents;
}

let counter = 0;

/**
 * Runs one streamed model call and resolves with the whole text, for
 * background jobs (rolling notes, summary) that don't show tokens as they
 * arrive. Rejects with the provider's error.
 */
export async function complete(
  deps: CompleteDeps,
  provider: string,
  request: LlmRequest,
  prefix = "job",
): Promise<string> {
  const requestId = `${prefix}:${Date.now().toString(36)}${(counter++).toString(36)}`;
  let text = "";
  let settle: { resolve: (t: string) => void; reject: (e: Error) => void } | null = null;
  const done = new Promise<string>((resolve, reject) => (settle = { resolve, reject }));
  // Listen before starting, so no token is missed.
  const unlisten = await Promise.all([
    deps.events.onToken((e) => {
      if (e.requestId === requestId) text += e.delta;
    }),
    deps.events.onDone((e) => {
      if (e.requestId === requestId) settle?.resolve(text);
    }),
    deps.events.onError((e) => {
      if (e.requestId === requestId) settle?.reject(new Error(e.error.message));
    }),
  ]);
  try {
    const started = await deps.llmStream(requestId, provider, request);
    if (started.status === "error") throw new Error(started.error.message);
    return await done;
  } finally {
    unlisten.forEach((u) => u());
  }
}
