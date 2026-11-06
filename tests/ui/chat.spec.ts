import { expect, test } from "@playwright/test";
import { expectAccessible, open, say, startSession } from "./helpers";

test.beforeEach(async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "The Pro tier allows 600 requests per minute.");
  await page.getByRole("tab", { name: "Chat" }).click();
});

test("asks a question, gets a cited answer, then a follow-up", async ({ page }) => {
  const box = page.getByRole("textbox", { name: "Ask a question" });
  await box.fill("What's the rate limit?");
  await box.press("Enter");
  const first = page.getByRole("article", { name: "Chat: What's the rate limit?" });
  await expect(first.getByText(/Mock answer to:/)).toBeVisible();
  await expect(first.getByRole("list", { name: "Sources" })).toContainText("Transcript");

  await box.fill("And for the Free tier?");
  await box.press("Enter");
  await expect(page.getByRole("article", { name: "Chat: And for the Free tier?" })).toBeVisible();
  await expect(page.getByRole("article")).toHaveCount(2);
  await expectAccessible(page);
});

test("Shift+Enter adds a line instead of sending", async ({ page }) => {
  const box = page.getByRole("textbox", { name: "Ask a question" });
  await box.fill("line one");
  await box.press("Shift+Enter");
  await box.pressSequentially("line two");
  await expect(box).toHaveValue("line one\nline two");
  await expect(page.getByRole("article")).toHaveCount(0);
});

test("capture-and-ask: screenshot, then a question about it", async ({ page }) => {
  await page.getByRole("button", { name: "Screenshot" }).click();
  await page.getByRole("menuitem", { name: "Full screen" }).click();
  await expect(page.getByRole("status", { name: "Screenshot attached" })).toContainText("1568×882");
  const box = page.getByRole("textbox", { name: "Ask a question" });
  await expect(box).toBeFocused();
  await box.fill("What does this error mean?");
  await box.press("Enter");
  const turn = page.getByRole("article", { name: "Chat: What does this error mean?" });
  await expect(turn.getByText(/Mock answer \(with screenshot\) to:/)).toBeVisible();
  await expect(turn.getByRole("list", { name: "Sources" })).toContainText("Screenshot");
});

test("the Ask shortcut opens Chat and focuses the input", async ({ page }) => {
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await page.evaluate(() =>
    (
      window as unknown as { __sidekickTest: { sessionBus: { emit(k: string, e: unknown): void } } }
    ).__sidekickTest.sessionBus.emit("hotkey", { action: "ask" }),
  );
  await expect(page.getByRole("tab", { name: "Chat" })).toHaveAttribute("aria-selected", "true");
  await expect(page.getByRole("textbox", { name: "Ask a question" })).toBeFocused();
});
