import { expect, test } from "@playwright/test";
import { open, say, startSession } from "./helpers";

// Chromium grants real clipboard access. WebKit (the macOS webview engine) has
// no clipboard permissions in Playwright, so it gets an in-memory clipboard
// with the same API: the app's copy button is exercised the same way.
test.beforeEach(async ({ context, browserName }) => {
  if (browserName === "chromium") {
    await context.grantPermissions(["clipboard-read", "clipboard-write"]);
    return;
  }
  await context.addInitScript(() => {
    let text = "";
    Object.defineProperty(navigator, "clipboard", {
      configurable: true,
      value: {
        writeText: (t: string) => ((text = t), Promise.resolve()),
        readText: () => Promise.resolve(text),
      },
    });
  });
});

test("code in an answer is highlighted and copied only on click", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "We use TypeScript.");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.evaluate(() => navigator.clipboard.writeText("untouched"));
  // The mock model echoes the question, so the fenced code comes back in the answer.
  const box = page.getByRole("textbox", { name: "Ask a question" });
  await box.fill("Explain this:\n```ts\nconst limit = 600;\n```\nThanks.");
  await box.press("Enter");
  const code = page.locator("pre code", { hasText: "const limit = 600;" }).first();
  await expect(code).toBeVisible();
  // Shiki highlights it (coloured spans) once loaded.
  await expect(page.locator("pre span[style*='color']").first()).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("untouched");

  await page.getByRole("button", { name: "Copy code" }).first().click();
  await expect(page.getByRole("button", { name: "Copied" })).toBeVisible();
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe("const limit = 600;");
});
