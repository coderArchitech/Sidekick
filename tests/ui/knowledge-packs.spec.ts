import { expect, test } from "@playwright/test";
import { expectAccessible, open, startSession, type SidekickTest } from "./helpers";

/** T-044: Knowledge Packs (PRD CTX-5) and the pre-meeting brief (CTX-4). */
test("Settings: create a pack and add a link", async ({ page }) => {
  await open(page, "settings");
  const section = page.getByRole("region", { name: "Knowledge Packs" });
  await section.getByLabel("New pack name").fill("Product specs");
  await section.getByRole("button", { name: "Create pack" }).click();
  await section.getByLabel("Link to add to Product specs").fill("https://example.com/spec");
  await section.getByRole("button", { name: "Add link" }).click();
  await expect(section.getByRole("list", { name: "Documents in Product specs" })).toContainText(
    "https://example.com/spec",
  );
  await expect(section.getByRole("list", { name: "Knowledge Packs" })).toContainText("1/1 ready");
  await expectAccessible(page);
});

test("Overlay: a pack turned on is searched, and the brief reads it", async ({ page }) => {
  await open(page);
  // Packs are made in Settings (another window, other mocks here): add one directly.
  await page.evaluate(async () => {
    const t = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    const p = await t.commands.createPack("Sales kit");
    await t.commands.ingestPackDocument(p.data.id, "/docs/prices.md");
  });
  await startSession(page);

  await page.getByRole("button", { name: /Documents \(/ }).click();
  const pack = page.getByRole("checkbox", { name: /Sales kit/ });
  await pack.check();
  await expect(pack).toBeChecked();
  await expectAccessible(page);

  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("textbox", { name: "Ask a question" }).fill("What does Pro cost?");
  await page.keyboard.press("Enter");
  const turn = page.getByRole("article", { name: "Chat: What does Pro cost?" });
  await expect(turn.getByRole("list", { name: "Sources" })).toContainText("prices.md");

  await page.getByRole("tab", { name: "Notes" }).click();
  const brief = page.getByRole("region", { name: "Pre-meeting brief" });
  await brief.getByRole("button", { name: "Write brief" }).click();
  // The mock model echoes the documents it was sent.
  await expect(brief).toContainText("Mock answer to:");
  await expect(brief.getByRole("button", { name: "Rewrite" })).toBeVisible();
  await expectAccessible(page);
});
