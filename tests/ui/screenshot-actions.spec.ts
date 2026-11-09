import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open, startSession } from "./helpers";

/** T-043: screenshot quick actions and strip (PRD SCR-5, SCR-6). */
async function captureScreen(page: Page) {
  await page.getByRole("button", { name: "Screenshot", exact: true }).click();
  await page.getByRole("menuitem", { name: "Full screen" }).click();
  await expect(page.getByRole("status", { name: "Screenshot attached" })).toBeVisible();
}

test("a quick action answers about the attached screenshot", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByRole("tab", { name: "Chat" }).click();
  await captureScreen(page);
  const actions = page.getByRole("group", { name: "Quick actions" });
  await expect(actions.getByRole("button")).toHaveText([
    "Explain",
    "Summarise",
    "Extract table",
    "Answer question on screen",
    "Debug this code",
  ]);
  await expectAccessible(page);

  await actions.getByRole("button", { name: "Extract table" }).click();
  const turn = page.getByRole("article", { name: "Chat: Extract table (screenshot)" });
  // The mock model notes the image and echoes the prompt.
  await expect(turn).toContainText("Mock answer (with screenshot) to:");
  await expect(turn).toContainText("Markdown table");
  await expect(page.getByRole("status", { name: "Screenshot attached" })).toHaveCount(0);
});

test("the strip keeps every capture, with re-ask and delete", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByRole("tab", { name: "Chat" }).click();
  await captureScreen(page);
  await captureScreen(page);
  await page.getByRole("button", { name: "Remove screenshot" }).click();

  const strip = page.getByRole("region", { name: "Screenshots" });
  await expect(strip).toContainText("Screenshots (2)");
  await expectAccessible(page);

  // Re-ask the first capture, then use a quick action on it.
  const asks = strip.getByRole("button", { name: /^Ask about the screenshot/ });
  await asks.first().click();
  await expect(asks.first()).toHaveAttribute("aria-pressed", "true");
  await page.getByRole("button", { name: "Debug this code" }).click();
  await expect(
    page.getByRole("article", { name: "Chat: Debug this code (screenshot)" }),
  ).toContainText("Mock answer (with screenshot) to:");

  await strip
    .getByRole("button", { name: /^Delete the screenshot/ })
    .first()
    .click();
  await expect(strip).toContainText("Screenshots (1)");
});
