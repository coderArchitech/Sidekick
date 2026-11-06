import { expect, test } from "@playwright/test";
import { expectAccessible, open } from "./helpers";

test.beforeEach(async ({ page }) => open(page));

test("consent dialog needs a choice, can be cancelled, and starts listening", async ({ page }) => {
  await page.getByLabel("Session name").fill("Interview prep");
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await expect(dialog.getByRole("button", { name: "Start listening" })).toBeDisabled();
  await expectAccessible(page);

  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("button", { name: "Start", exact: true })).toBeVisible();

  await page.getByRole("button", { name: "Start", exact: true }).click();
  await dialog.getByLabel("Transcribe me only").check();
  await dialog.getByRole("button", { name: "Start listening" }).click();
  await expect(page.getByText("Listening (you only)")).toBeVisible();
});

test("the consent dialog works from the keyboard", async ({ page }) => {
  await page.getByLabel("Session name").fill("Keyboard");
  await page.keyboard.press("Enter");
  await page.getByRole("button", { name: "Start", exact: true }).focus();
  await page.keyboard.press("Enter");
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await expect(dialog).toBeVisible();
  // Radios: focus the first option, arrow to the second, then tab to Start listening.
  await dialog.getByLabel("Participants know and agree to transcription").focus();
  await page.keyboard.press("Space");
  await page.keyboard.press("ArrowDown");
  await expect(dialog.getByLabel("Transcribe me only")).toBeChecked();
  await dialog.getByRole("button", { name: "Start listening" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("Listening (you only)")).toBeVisible();
});
