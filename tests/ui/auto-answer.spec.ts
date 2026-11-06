import { expect, test } from "@playwright/test";
import { open, say, startSession, type SidekickTest } from "./helpers";

test.beforeEach(async ({ page }) => {
  await open(page);
  await page.evaluate(() =>
    (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest.commands.updateSettings({
      profile: { name: "Priya", nicknames: [] },
    }),
  );
  await startSession(page);
});

test("with Auto on, every detected question is answered", async ({ page }) => {
  const auto = page.getByRole("button", { name: "Auto" });
  await expect(auto).toHaveAttribute("aria-pressed", "true");
  await say(page, "What's the rate limit for the Pro tier?");
  const card = page.getByRole("article", {
    name: "Answer to: What's the rate limit for the Pro tier?",
  });
  await expect(card).toBeVisible();
  await expect(card.getByText(/Mock answer to:/)).toBeVisible();
  await expect(card.getByRole("button", { name: "Helpful", exact: true })).toBeVisible();
});

test("with Auto off, only questions that use your name are answered", async ({ page }) => {
  await page.getByRole("button", { name: "Auto" }).click();
  await expect(page.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByText("Listening for questions that use your name…")).toBeVisible();

  await say(page, "What's the budget for next quarter?");
  // Past the 600 ms detection debounce, so the two aren't merged into one question.
  await page.waitForTimeout(1000);
  await say(page, "Priya, can you share the timeline?");
  await expect(
    page.getByRole("article", { name: "Answer to: Priya, can you share the timeline?" }),
  ).toBeVisible();
  await expect(page.getByText("For you")).toBeVisible();
  await expect(page.getByRole("article")).toHaveCount(1);
});

test("the Auto shortcut toggles it too", async ({ page }) => {
  await page.evaluate(() =>
    (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest.sessionBus.emit(
      "hotkey",
      {
        action: "auto_answer",
      },
    ),
  );
  await expect(page.getByRole("button", { name: "Auto" })).toHaveAttribute("aria-pressed", "false");
});
