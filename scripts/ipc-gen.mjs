// Regenerates src/ipc/bindings.ts from the Rust command list (tech doc 5.6).
// A Node script rather than an inline env var so it works in Windows shells too.
import { execFileSync } from "node:child_process";

const run = (cmd, args, env = {}) =>
  execFileSync(cmd, args, {
    stdio: "inherit",
    env: { ...process.env, ...env },
    shell: process.platform === "win32",
  });

run("cargo", ["test", "-p", "sidekick-app", "--lib", "ipc::tests::export_typescript_bindings"], {
  SIDEKICK_IPC_GEN: "1",
});
run("pnpm", ["exec", "prettier", "--write", "src/ipc/bindings.ts"]);
