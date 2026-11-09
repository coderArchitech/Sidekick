import { expect, test } from "@playwright/test";
import { expectAccessible, open } from "./helpers";

/** Settings → Diagnostics (T-012) and log export (T-031). Baselines on real laptops are manual. */

type Hook = { files: { saveAs: string[] } };

test("the latency table lists every stage with its target", async ({ page }) => {
  await open(page, "settings");
  const panel = page.getByRole("region", { name: "Diagnostics" });
  const table = panel.getByRole("table", { name: "Latency by stage" });
  for (const stage of [
    "Transcript lag",
    "Question to cue",
    "Question to full answer",
    "LLM first token",
    "Context retrieval",
  ])
    await expect(table.getByRole("rowheader", { name: stage })).toBeVisible();
  await expect(table.getByRole("row")).toHaveCount(6);
  await expectAccessible(page, "#diagnostics-heading");
});

test("exporting logs reports the result, and cancelling does nothing", async ({ page }) => {
  await open(page, "settings");
  const panel = page.getByRole("region", { name: "Diagnostics" });
  await panel.getByRole("button", { name: "Export logs…" }).click(); // no path: cancelled
  await expect(panel.getByRole("status")).toHaveCount(0);

  await page.evaluate(() => {
    (window as unknown as { __sidekickTest: Hook }).__sidekickTest.files.saveAs = ["/tmp/logs.zip"];
  });
  await panel.getByRole("button", { name: "Export logs…" }).click();
  await expect(panel.getByRole("status")).toContainText("Saved the log bundle");
});
