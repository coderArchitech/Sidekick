import { test } from "@playwright/test";
import { expectAccessible, open, say, startSession } from "./helpers";

test("every overlay tab passes axe, empty and with content", async ({ page }) => {
  await open(page);
  for (const tab of ["Live Answers", "Transcript", "Notes", "Chat"]) {
    await page.getByRole("tab", { name: tab }).click();
    await expectAccessible(page);
  }
  await startSession(page);
  await say(page, "What's the rate limit for the Pro tier?");
  await page.getByRole("tab", { name: "Live Answers" }).click();
  await page.getByRole("article").first().waitFor();
  for (const tab of ["Live Answers", "Transcript", "Notes", "Chat"]) {
    await page.getByRole("tab", { name: tab }).click();
    await expectAccessible(page);
  }
});

test("compact mode and the document panel pass axe", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByText(/Documents \(/).click();
  await expectAccessible(page);
  await page.getByRole("button", { name: "Compact" }).click();
  await expectAccessible(page);
});
