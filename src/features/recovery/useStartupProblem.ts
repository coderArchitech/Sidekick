import { useEffect, useState } from "react";
import { commands, type StartupProblem } from "@/ipc";

/**
 * Why startup failed: `undefined` while checking, `null` when Sidekick started
 * normally, otherwise the problem for the recovery screen.
 */
export function useStartupProblem(): StartupProblem | null | undefined {
  const [problem, setProblem] = useState<StartupProblem | null | undefined>(undefined);
  useEffect(() => {
    let live = true;
    void commands.startupProblem().then(
      (p) => live && setProblem(p),
      // An older core without the command: carry on as a normal start.
      () => live && setProblem(null),
    );
    return () => {
      live = false;
    };
  }, []);
  return problem;
}
