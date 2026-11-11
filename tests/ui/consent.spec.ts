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
  await dialog.getByLabel("Only me").check();
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
  // Radios: focus "The other people and me", arrow to "Only me", then Start listening.
  await dialog.getByLabel("The other people and me").focus();
  await page.keyboard.press("Space");
  await page.keyboard.press("ArrowDown");
  await expect(dialog.getByLabel("Only me")).toBeChecked();
  await dialog.getByRole("button", { name: "Start listening" }).focus();
  await page.keyboard.press("Enter");
  await expect(page.getByText("Listening (you only)")).toBeVisible();
});

test("the other people only, or the other people and me with a Me switch", async ({ page }) => {
  await page.getByLabel("Session name").fill("Interview");
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await expect(
    dialog.getByRole("group", { name: "Who should Sidekick transcribe?" }),
  ).toBeVisible();
  await dialog.getByLabel("The other people only").check();
  await dialog.getByRole("button", { name: "Start listening" }).click();
  await expect(page.getByText("Listening (others only)")).toBeVisible();
  await expect(page.getByRole("button", { name: "Me", exact: true })).toHaveCount(0);
  await page.getByRole("button", { name: "End" }).click();

  await page.getByLabel("Session name").fill("Interview, me too");
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  await dialog.getByLabel("The other people and me").check();
  await dialog.getByRole("button", { name: "Start listening" }).click();
  const me = page.getByRole("button", { name: "Me", exact: true });
  await expect(me).toHaveAttribute("aria-pressed", "true");
  await me.click();
  await expect(me).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByText("Listening (others only)")).toBeVisible();
  await expectAccessible(page);
  await me.click();
  await expect(me).toHaveAttribute("aria-pressed", "true");
});
