import { expect, test } from "@playwright/test";
import { open } from "./helpers";

/**
 * Screen-share protection (T-011, T-012): the shield in the overlay header and
 * the status line in Settings → Diagnostics. Whether each meeting app really
 * hides the window is a manual check per OS.
 */

type Hook = { protection: { status: { state: string; reason: string } } };

test("the header shield says plainly when the overlay is visible in a share", async ({ page }) => {
  await open(page);
  const shield = page.getByRole("status", { name: /Visible in screen share/ });
  await expect(shield).toBeVisible();
  await expect(shield).toHaveAttribute("title", /no screen-share protection/);
});

for (const [state, label] of [
  ["protected", "Hidden from screen share"],
  ["not_guaranteed", "Screen-share hiding not guaranteed"],
  ["disabled", "Screen-share hiding off"],
] as const) {
  test(`Diagnostics shows the ${state} reason after Refresh`, async ({ page }) => {
    await open(page, "settings");
    await page.evaluate(
      ([s, r]) => {
        (window as unknown as { __sidekickTest: Hook }).__sidekickTest.protection.status = {
          state: s as string,
          reason: r as string,
        };
      },
      [state, `Reason for ${label}`],
    );
    const panel = page.getByRole("region", { name: "Diagnostics" });
    await panel.getByRole("button", { name: "Refresh" }).click();
    await expect(panel).toContainText(`Reason for ${label}`);
  });
}
