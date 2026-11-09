import { expect, test } from "@playwright/test";
import { open, startSession } from "./helpers";

/**
 * Overlay display controls (T-029): text size, background and Compact.
 * Snap-to-edge and monitor restore are native and checked by hand.
 */

test("text size steps within its limits", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "View" }).click();
  const menu = page.getByRole("dialog", { name: "View options" });
  const smaller = menu.getByRole("button", { name: "Smaller text" });
  const larger = menu.getByRole("button", { name: "Larger text" });

  await larger.click();
  const after = await menu.getByText(/^\d+px$/).textContent();
  const size = Number(after?.replace("px", ""));
  const root = () => page.locator("[data-compact], .glass").first();
  expect(size).toBeGreaterThan(12);
  expect(await root().count()).toBeGreaterThan(0);

  for (let i = 0; i < 12; i++) if (await smaller.isEnabled()) await smaller.click();
  await expect(menu.getByText("12px")).toBeVisible();
  await expect(smaller).toBeDisabled();
  for (let i = 0; i < 12; i++) if (await larger.isEnabled()) await larger.click();
  await expect(menu.getByText("20px")).toBeVisible();
  await expect(larger).toBeDisabled();
});

test("Compact hides the session bar and documents, and brings them back", async ({ page }) => {
  await open(page);
  await startSession(page);
  const docs = page.getByRole("button", { name: /Documents \(/ });
  await expect(docs).toBeVisible();

  await page.getByRole("button", { name: "View" }).click();
  await page.getByRole("switch", { name: "Compact" }).check();
  await page.keyboard.press("Escape");
  await expect(docs).toBeHidden();
  await expect(page.locator("[data-compact]")).toBeVisible();
  await expect(page.getByRole("button", { name: "Pause" })).toBeHidden();

  await page.getByRole("button", { name: "View" }).click();
  await page.getByRole("switch", { name: "Compact" }).uncheck();
  await page.keyboard.press("Escape");
  await expect(docs).toBeVisible();
});

test("the background slider is limited to 20–100%", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "View" }).click();
  const slider = page.getByRole("slider", { name: "Background opacity" });
  await expect(slider).toHaveAttribute("min", "0.2");
  await expect(slider).toHaveAttribute("max", "1");
  await slider.fill("0.2");
  await expect(page.getByRole("dialog", { name: "View options" })).toContainText("20%");
});
