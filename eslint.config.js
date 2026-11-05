import js from "@eslint/js";
import jsxA11y from "eslint-plugin-jsx-a11y";
import reactHooks from "eslint-plugin-react-hooks";
import globals from "globals";
import tseslint from "typescript-eslint";

// Module boundaries (tech doc 7.8) are enforced here, not just documented.
const boundaries = {
  patterns: [
    {
      group: ["@/features/*/*", "**/features/*/*"],
      message: "Import another feature only through its public index: '@/features/<name>'.",
    },
    {
      group: ["@/ipc/*", "**/ipc/bindings", "**/ipc/mock"],
      message: "Import IPC only through '@/ipc'.",
    },
  ],
  paths: [
    {
      name: "@tauri-apps/api/core",
      message: "Components never call invoke directly; use the typed commands from '@/ipc'.",
    },
  ],
};

export default tseslint.config(
  { ignores: ["dist", "target", "src-tauri", "crates", "src/ipc/bindings.ts", "coverage"] },
  js.configs.recommended,
  ...tseslint.configs.strict,
  jsxA11y.flatConfigs.recommended,
  {
    files: ["**/*.{ts,tsx}"],
    languageOptions: { globals: globals.browser },
    plugins: { "react-hooks": reactHooks },
    rules: {
      ...reactHooks.configs.recommended.rules,
      "@typescript-eslint/no-explicit-any": "error",
      "no-restricted-imports": ["error", boundaries],
    },
  },
  {
    files: ["scripts/**/*.mjs"],
    languageOptions: { globals: globals.node },
  },
  {
    // The ipc module is the one place allowed to touch its own internals and Tauri.
    files: ["src/ipc/**"],
    rules: { "no-restricted-imports": "off" },
  },
);
