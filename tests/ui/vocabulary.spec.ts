import { expect, test } from "@playwright/test";
import { expectAccessible, open } from "./helpers";

/** T-047: custom vocabulary in Settings (PRD LIS-5). */
test("vocabulary is saved cleaned, with a count, and validated", async ({ page }) => {
  await open(page, "settings");
  const section = page.getByRole("region", { name: "Custom vocabulary" });
  const box = section.getByLabel("Vocabulary terms");
  await box.fill("Kubernetes, SOC 2\n kubernetes \nPriya   Raman");
  await expect(section).toContainText("4/100 terms");
  await section.getByRole("button", { name: "Save vocabulary" }).click();
  await expect(section.getByRole("status")).toHaveText(
    "Saved 3 terms. They apply from the next session.",
  );
  await expect(box).toHaveValue("Kubernetes\nSOC 2\nPriya Raman");
  await expectAccessible(page);

  await box.fill("x".repeat(51));
  await section.getByRole("button", { name: "Save vocabulary" }).click();
  await expect(section.getByRole("alert")).toContainText("up to 100 terms");
});
