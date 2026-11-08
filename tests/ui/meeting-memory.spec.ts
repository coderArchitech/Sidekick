import { expect, test } from "@playwright/test";
import { expectAccessible, open, say, startSession, type SidekickTest } from "./helpers";

/** T-041: agenda tracking, "What did I miss?" and action items (PRD MEM-2 to MEM-4). */
test("agenda moves on as the meeting goes, and closes when it ends", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByRole("tab", { name: "Notes" }).click();

  const agenda = page.getByRole("region", { name: "Agenda" });
  await agenda.getByRole("button", { name: "Add agenda" }).click();
  await agenda.getByLabel("Agenda items, one per line").fill("Pro tier pricing\nDashboard\nHiring");
  await agenda.getByRole("button", { name: "Save agenda" }).click();
  await expect(agenda).toContainText("0/3 covered");
  await expectAccessible(page);

  await say(page, "What's the rate limit for the Pro tier?");
  await say(page, "Let's also look at the dashboard.");

  // Ending runs a last notes update (the mock model's reply isn't JSON, so
  // keywords move the agenda), then closes the item still in progress.
  await page.getByRole("button", { name: "End", exact: true }).click();
  await expect(agenda).toContainText("2/3 covered");
  await expect(agenda.getByLabel("Status of Pro tier pricing")).toHaveValue("covered");
  await expect(agenda.getByLabel("Status of Dashboard")).toHaveValue("covered");
  await expect(agenda.getByLabel("Status of Hiring")).toHaveValue("not_started");

  // A manual change sticks.
  await agenda.getByLabel("Status of Hiring").selectOption("in_progress");
  await expect(agenda.getByLabel("Status of Hiring")).toHaveValue("in_progress");
});

test("'What did I miss?' recaps the last minutes", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "Priya, can you send the quote by Friday?");
  await page.getByRole("tab", { name: "Notes" }).click();
  await page.getByLabel("Recap period").selectOption("2");
  await page.getByRole("button", { name: "What did I miss?" }).click();
  const card = page.getByRole("region", { name: "What you missed" });
  await expect(card).toContainText("Last 2 minutes");
  // The mock model echoes what it was sent: the recent transcript.
  await expect(card).toContainText("can you send the quote by Friday?");
  await expectAccessible(page);
  await card.getByRole("button", { name: "Dismiss" }).click();
  await expect(card).toHaveCount(0);
});

test("action items can be ticked off", async ({ page }) => {
  await open(page);
  await startSession(page);
  await say(page, "Priya will send the quote by Friday.");
  await page.getByRole("button", { name: "End", exact: true }).click();
  await page.getByRole("tab", { name: "Notes" }).click();
  await expect(page.getByRole("region", { name: "Summary" })).toBeVisible();
  // The mock model's summary isn't JSON, so store the action items directly;
  // the Notes tab reloads them when it opens.
  await page.getByRole("tab", { name: "Transcript" }).click();
  await page.evaluate(async () => {
    const t = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    const r = await t.commands.listSessions(null, null);
    const id = r.status === "ok" ? r.data.items[0]?.id : undefined;
    if (!id) throw new Error("no session");
    await t.commands.saveSummary(id, "We agreed on the Pro tier.", [
      { text: "Send the quote", owner: "Priya", due: "Friday" },
    ]);
  });
  await page.getByRole("tab", { name: "Notes" }).click();
  const box = page.getByRole("checkbox", { name: /Send the quote/ });
  await expect(box).toBeVisible();
  await box.check();
  await expect(box).toBeChecked();
});
