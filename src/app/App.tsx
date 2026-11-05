import { Overlay } from "@/features/overlay";

/**
 * App shell. Each Tauri window renders one top-level feature; the Settings
 * window will be added here once the settings feature exists.
 */
export function App() {
  return <Overlay />;
}
