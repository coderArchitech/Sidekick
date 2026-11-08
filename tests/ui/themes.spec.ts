import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open, say, startSession, type SidekickTest } from "./helpers";

/** T-048: themes pass axe with contrast (PRD UI-8); click-through (UI-5). */
const THEMES = [
  ["light", "light"],
  ["high_contrast", "high-contrast"],
  ["dark", "dark"],
] as const;

async function setTheme(page: Page, theme: string) {
  await page.evaluate(async (t) => {
    const s = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    await s.commands.updateSettings({ theme: t });
  }, theme);
}

for (const [theme, attr] of THEMES) {
  test(`overlay in ${theme} passes axe on every tab`, async ({ page }) => {
    await open(page);
    await setTheme(page, theme);
    await expect(page.locator("html")).toHaveAttribute("data-theme", attr);
    await startSession(page);
    await say(page, "What's the rate limit for the Pro tier?");
    await page.getByRole("tab", { name: "Live Answers" }).click();
    await page.getByRole("article").first().waitFor();
    for (const tab of ["Live Answers", "Transcript", "Notes", "Chat"]) {
      await page.getByRole("tab", { name: tab }).click();
      await expectAccessible(page);
    }
  });

  test(`Settings in ${theme} passes axe`, async ({ page }) => {
    await open(page, "settings");
    await page.getByRole("combobox", { name: "Theme" }).selectOption(theme);
    await expect(page.locator("html")).toHaveAttribute("data-theme", attr);
    await expectAccessible(page);
  });
}

test("click-through: View menu on, notice shown, shortcut off", async ({ page }) => {
  await open(page);
  await page.getByRole("button", { name: "View" }).click();
  // The menu closes on the click, so the panel is left just visible.
  await page.getByRole("switch", { name: "Click-through" }).click();
  const notice = page.getByRole("status").filter({ hasText: "Click-through is on" });
  await expect(notice).toContainText("Ctrl/Cmd+Shift+T turns it off");
  await expectAccessible(page);
  // The global shortcut arrives as a hotkey event.
  await page.evaluate(() => {
    const s = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    s.sessionBus.emit("hotkey", { action: "click_through" });
  });
  await expect(notice).toHaveCount(0);
});
