import { defineConfig, devices } from "@playwright/test";

// README screenshots (pnpm docs:screenshots): the real frontend with the
// mocked backend and canned model replies, saved to docs/screenshots/app/.
const PORT = 4175;

export default defineConfig({
  testDir: "tests/screenshots",
  outputDir: "test-results/screenshots",
  fullyParallel: true,
  reporter: [["list"]],
  use: {
    ...devices["Desktop Chrome"],
    baseURL: `http://localhost:${PORT}`,
    deviceScaleFactor: 2,
    trace: "off",
  },
  webServer: {
    command: `pnpm exec vite --mode uitest --port ${PORT} --strictPort`,
    port: PORT,
    reuseExistingServer: true,
  },
});
