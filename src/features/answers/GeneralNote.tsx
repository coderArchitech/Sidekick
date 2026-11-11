import { GENERAL_LABEL } from "@/orchestrator";

/**
 * Marks an answer that came from the model's general knowledge because the
 * meeting and documents didn't cover it: it has no sources to check.
 */
export function GeneralNote() {
  return (
    <p className="mt-1 flex items-center gap-1.5 text-xs text-warning">
      <span aria-hidden="true" className="inline-block h-1.5 w-1.5 rounded-full bg-warning" />
      {GENERAL_LABEL}
    </p>
  );
}
