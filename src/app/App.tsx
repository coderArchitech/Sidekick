import { Overlay } from "@/features/overlay";
import { SettingsWindow } from "@/features/settings";
import { currentWindow } from "@/ipc";
import { useAppTheme } from "@/lib/theme";

/**
 * App shell. Each Tauri window loads the same bundle and renders one
 * top-level feature, chosen by the `?window=` query in tauri.conf.json.
 */
export function App() {
  useAppTheme();
  return currentWindow() === "settings" ? <SettingsWindow /> : <Overlay />;
}
