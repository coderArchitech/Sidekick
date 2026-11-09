import { expect, test } from "@playwright/test";
import { choose, expectAccessible, open, say, startSession } from "./helpers";

/** Reported issues 2–5 in the browser build (1 is OS window behaviour: E2E and manual). */
test("2: dropdowns open inside the window, never as native popups", async ({ page }) => {
  await open(page);
  await startSession(page);
  await expect(page.locator("select")).toHaveCount(0);
  const picker = page.getByRole("combobox", { name: "Skill" });
  await picker.click();
  const list = page.getByRole("listbox", { name: "Skill" });
  await expect(list).toBeVisible();
  await expectAccessible(page);
  // Keyboard: down to the next Skill, Enter picks it.
  await page.keyboard.press("ArrowDown");
  await page.keyboard.press("Enter");
  await expect(list).toHaveCount(0);
  await expect(picker).not.toHaveAttribute("data-value", "");

  await open(page, "settings");
  await expect(page.locator("select")).toHaveCount(0);
});

test("3: the Staff Engineer Interview (JS) Skill is built in and used for answers", async ({
  page,
}) => {
  await open(page);
  await startSession(page);
  await choose(page.getByRole("combobox", { name: "Skill" }), "staff-engineer-interview");
  await expect(page.getByRole("combobox", { name: "Skill" })).toHaveText(
    /Staff Engineer Interview \(JS\)/,
  );
});

test("4: a screenshot on Live Answers is answered there as the question", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await page.getByRole("button", { name: "Screenshot", exact: true }).click();
  await page.getByRole("menuitem", { name: "Full screen" }).click();
  const card = page.getByRole("article", { name: "Answer to: Question on screen" });
  await expect(card).toContainText("Mock answer (with screenshot) to:");
  await expect(page.getByRole("tab", { name: "Live Answers" })).toHaveAttribute(
    "aria-selected",
    "true",
  );
  await expectAccessible(page);
});

test("5: the other person's tasks are detected and answered", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "Okay so design a URL shortener like bitly");
  // A pause ends the turn (lines close together are read as one question).
  await page.waitForTimeout(900);
  await say(page, "Implement a debounce function in JavaScript.");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await expect(
    page.getByRole("article", { name: "Answer to: Okay so design a URL shortener like bitly" }),
  ).toBeVisible();
  await expect(
    page.getByRole("article", { name: "Answer to: Implement a debounce function in JavaScript." }),
  ).toBeVisible();
});
