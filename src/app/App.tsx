import { Overlay } from "@/features/overlay";
import { RecoveryScreen, useStartupProblem } from "@/features/recovery";
import { SettingsWindow } from "@/features/settings";
import { currentWindow } from "@/ipc";
import { useAppTheme } from "@/lib/theme";

/**
 * App shell. Each Tauri window loads the same bundle and renders one
 * top-level feature, chosen by the `?window=` query in tauri.conf.json.
 * If startup failed, the overlay shows the recovery screen instead and the
 * Settings window stays empty (tech doc 4.5).
 */
export function App() {
  useAppTheme();
  const problem = useStartupProblem();
  if (problem === undefined) return null;
  if (currentWindow() === "settings") return problem ? null : <SettingsWindow />;
  return problem ? <RecoveryScreen problem={problem} /> : <Overlay />;
}
