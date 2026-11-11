import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open } from "./helpers";

/**
 * Recovery screen (tech doc 4.5): shown instead of the overlay when startup
 * fails. The problem is injected before the page loads, as the core would
 * report it.
 */

type Problem = {
  kind: "key_missing" | "keychain_unavailable" | "database_unreadable" | "other";
  message: string;
  dataDir: string;
  hasBackup: boolean;
  keychainName: string;
};

const keyMissing: Problem = {
  kind: "key_missing",
  message: "The key that unlocks Sidekick's data is missing from the keychain.",
  dataDir: "/Users/me/Library/Application Support/com.sidekick.app",
  hasBackup: false,
  keychainName: "Keychain",
};

async function openWithProblem(page: Page, problem: Problem) {
  await page.addInitScript((p) => {
    (window as unknown as { __sidekickStartupProblem: unknown }).__sidekickStartupProblem = p;
  }, problem);
  await open(page);
}

test("a missing key shows the recovery screen instead of the overlay", async ({ page }) => {
  await openWithProblem(page, keyMissing);
  await expect(
    page.getByRole("heading", { name: "Sidekick can't unlock your saved data" }),
  ).toBeVisible();
  await expect(page.getByRole("tablist")).toHaveCount(0);
  await expect(page.getByRole("button", { name: "New session" })).toHaveCount(0);
  await expectAccessible(page);
});

test("starting fresh asks first, names where the old file stays, then restarts", async ({
  page,
}) => {
  await openWithProblem(page, keyMissing);
  await page.getByRole("button", { name: "Start fresh…" }).click();
  const confirm = page.getByRole("group", { name: "Confirm start fresh" });
  await expect(confirm).toContainText(keyMissing.dataDir);
  await expectAccessible(page);
  await confirm.getByRole("button", { name: "Move it aside and start fresh" }).click();
  await expect(page.getByRole("status")).toHaveText("Restarting Sidekick…");
});

test("an unreadable database with a backup offers Restore backup first", async ({ page }) => {
  await openWithProblem(page, {
    ...keyMissing,
    kind: "database_unreadable",
    message: "Couldn't open the database: wrong key or damaged file.",
    hasBackup: true,
  });
  await expect(
    page.getByRole("heading", { name: "Sidekick can't open its data file" }),
  ).toBeVisible();
  await expect(page.getByRole("button").first()).toHaveText("Restore backup");
  await page.getByRole("button", { name: "Restore backup" }).click();
  await expect(page.getByRole("status")).toHaveText("Restarting Sidekick…");
});

test("a locked keychain only offers Try again and Export logs", async ({ page }) => {
  await openWithProblem(page, { ...keyMissing, kind: "keychain_unavailable" });
  await expect(page.getByText("Your data is safe.", { exact: false })).toBeVisible();
  await expect(page.getByRole("button", { name: "Start fresh…" })).toHaveCount(0);
  await expect(page.getByRole("button", { name: "Try again" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Export logs…" })).toBeVisible();
  await expectAccessible(page);
});

test("the overlay opens normally when startup succeeded", async ({ page }) => {
  await open(page);
  await expect(page.getByRole("tablist")).toBeVisible();
  await expect(page.getByRole("heading", { name: /Sidekick can't/ })).toHaveCount(0);
});
