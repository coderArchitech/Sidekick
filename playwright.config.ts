import { defineConfig, devices } from "@playwright/test";

// UI tests against the real frontend with the mocked backend (tech doc 8.1,
// ADR-010). Full-app tests with the Rust core are WebdriverIO (T-033).
const PORT = 4174;

export default defineConfig({
  testDir: "tests/ui",
  outputDir: "test-results/ui",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [["list"], ["html", { outputFolder: "test-results/ui-report", open: "never" }]],
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "retain-on-failure",
    screenshot: "only-on-failure",
  },
  projects: [
    {
      // The overlay's default size (tauri.conf.json).
      name: "overlay",
      use: { ...devices["Desktop Chrome"], viewport: { width: 420, height: 640 } },
      testIgnore: /settings\.spec/,
    },
    {
      name: "settings",
      use: { ...devices["Desktop Chrome"], viewport: { width: 720, height: 900 } },
      testMatch: /settings\.spec/,
    },
  ],
  webServer: {
    command: `pnpm exec vite --mode uitest --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: !process.env.CI,
  },
});
