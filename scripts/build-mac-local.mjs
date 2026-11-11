// Builds the macOS app signed with the local "Sidekick Local Signing"
// certificate (created by scripts/macos-local-signing.sh). A stable signature
// keeps the keychain's "Always Allow" and the Microphone permission across
// rebuilds; unsigned local builds look like a new app to macOS every time.
// Releases are signed with Developer ID instead (T-037).
import { execFileSync } from "node:child_process";

const IDENTITY = "Sidekick Local Signing";

if (process.platform !== "darwin") {
  console.error("build:mac-local only runs on macOS.");
  process.exit(1);
}
try {
  execFileSync("security", ["find-certificate", "-c", IDENTITY], { stdio: "ignore" });
} catch {
  console.error(`No "${IDENTITY}" certificate. Run scripts/macos-local-signing.sh once first.`);
  process.exit(1);
}
execFileSync("pnpm", ["tauri", "build", ...process.argv.slice(2)], {
  stdio: "inherit",
  env: { ...process.env, APPLE_SIGNING_IDENTITY: IDENTITY },
});
