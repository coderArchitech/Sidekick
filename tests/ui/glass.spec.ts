import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open, say, startSession } from "./helpers";

/**
 * Glass overlay (T-049): the background is a translucent tint set by the
 * opacity setting, and the screen behind shows through it.
 */

/** A bright, busy stand-in for a video call behind the transparent window. */
async function addBackdrop(page: Page) {
  await page.addStyleTag({
    content: `html { background:
      radial-gradient(circle at 30% 35%, #f2c9a0 0 12%, transparent 13%),
      radial-gradient(circle at 72% 60%, #9fd3f5 0 14%, transparent 15%),
      linear-gradient(135deg, #3b6fd6 0 33%, #e9edf3 33% 66%, #2f9e6b 66%) !important; }`,
  });
}

const panel = (page: Page) => page.locator(".glass").first();

async function backgroundAlpha(page: Page): Promise<number> {
  return panel(page).evaluate((el) => {
    const bg = getComputedStyle(el).backgroundColor;
    const parts = (bg.match(/[\d.]+/g) ?? []).map(Number);
    return parts.length === 4 ? (parts[3] ?? 1) : 1;
  });
}

async function setBackground(page: Page, value: string) {
  await page.getByRole("button", { name: "View" }).click();
  await page.getByRole("slider", { name: "Background opacity" }).fill(value);
  await page.keyboard.press("Escape");
}

test("the opacity setting tints only the background, never the text", async ({
  page,
}, testInfo) => {
  await open(page);
  await addBackdrop(page);
  await startSession(page);
  await say(page, "What's the rate limit for the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await page.getByRole("article").first().waitFor();

  expect(await backgroundAlpha(page)).toBeCloseTo(0.8);
  await setBackground(page, "0.4");
  expect(await backgroundAlpha(page)).toBeCloseTo(0.4);
  // Nothing in the panel is faded: text and the panel itself stay opaque.
  expect(await panel(page).evaluate((el) => getComputedStyle(el).opacity)).toBe("1");
  const textColor = await page
    .getByRole("article")
    .first()
    .evaluate((el) => getComputedStyle(el).color);
  expect(textColor).toMatch(/^rgb\(/);
  await testInfo.attach("glass-40", { body: await page.screenshot(), contentType: "image/png" });

  await setBackground(page, "1");
  expect(await backgroundAlpha(page)).toBe(1);
  await testInfo.attach("glass-100", { body: await page.screenshot(), contentType: "image/png" });
});

test("at full opacity the panel passes axe over a busy backdrop", async ({ page }) => {
  await open(page);
  await addBackdrop(page);
  await setBackground(page, "1");
  for (const tab of ["Live Answers", "Transcript", "Notes", "Chat"]) {
    await page.getByRole("tab", { name: tab }).click();
    await expectAccessible(page);
  }
});
