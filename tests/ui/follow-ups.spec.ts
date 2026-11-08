import { expect, test } from "@playwright/test";
import { expectAccessible, open, say, startSession } from "./helpers";

/** T-042: follow-up chips under answers (PRD QA-9). */
test("a chip on a live answer asks its follow-up in Chat", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "What's the rate limit for the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  const card = page.getByRole("article", {
    name: "Answer to: What's the rate limit for the Pro tier?",
  });
  const chips = card.getByRole("group", { name: "Follow-ups" });
  await expect(chips.getByRole("button")).toHaveText([
    "Shorter",
    "More detail",
    "Give an example",
    "Explain simply",
  ]);
  await expectAccessible(page);

  await chips.getByRole("button", { name: "Shorter" }).click();
  await expect(page.getByRole("tab", { name: "Chat" })).toHaveAttribute("aria-selected", "true");
  const turn = page.getByRole("article", {
    name: "Chat: Shorter: What's the rate limit for the Pro tier?",
  });
  // The mock model echoes the instruction it was sent.
  await expect(turn).toContainText("at most two short sentences");
  await expectAccessible(page);
});

test("chips on a chat answer chain back to the first question", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "The Pro tier allows 600 requests per minute.");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("textbox", { name: "Ask a question" }).fill("What's the Pro limit?");
  await page.keyboard.press("Enter");
  const first = page.getByRole("article", { name: "Chat: What's the Pro limit?" });
  await first.getByRole("button", { name: "Give an example" }).click();
  const example = page.getByRole("article", {
    name: "Chat: Give an example: What's the Pro limit?",
  });
  await expect(example).toContainText("concrete, realistic example");
  await example.getByRole("button", { name: "More detail" }).click();
  await expect(
    page.getByRole("article", { name: "Chat: More detail: What's the Pro limit?" }),
  ).toContainText("more detail");
});
