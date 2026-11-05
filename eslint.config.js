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
      group: ["@/orchestrator/*"],
      message: "Import the orchestrator only through '@/orchestrator'.",
    },
    {
      group: ["@/ipc/*", "!@/ipc/testing", "**/ipc/bindings", "**/ipc/mock"],
      message: "Import IPC only through '@/ipc' (tests may also use '@/ipc/testing').",
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
    // Mock helpers are for tests only.
    files: ["src/**/*.{ts,tsx}"],
    ignores: ["src/**/*.test.{ts,tsx}", "src/ipc/**"],
    rules: {
      "no-restricted-imports": [
        "error",
        {
          ...boundaries,
          patterns: [
            ...boundaries.patterns,
            { group: ["@/ipc/testing"], message: "'@/ipc/testing' is for tests only." },
          ],
        },
      ],
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
