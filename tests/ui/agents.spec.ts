import { expect, test } from "@playwright/test";
import { expectAccessible, open, startSession } from "./helpers";

/** T-045: agent Skills use tools (PRD SKL-4); several Skills at once (SKL-5). */
test("an agent Skill uses the calculator before answering", async ({ page }) => {
  await open(page);
  await startSession(page);
  await page.getByRole("combobox", { name: "Skill" }).selectOption("negotiation-coach");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page
    .getByRole("textbox", { name: "Ask a question" })
    .fill("What is 2400 * 15% of the deal?");
  await page.keyboard.press("Enter");

  const turn = page.getByRole("article", { name: "Chat: What is 2400 * 15% of the deal?" });
  await expect(turn.getByRole("list", { name: "Tools used" })).toHaveText(
    "Calculator: 2400 * 15% = 360",
  );
  // The mock model echoes the prompt: the result is in the answer's context.
  await expect(turn).toContainText("calculator(2400 * 15%) → 360");
  await expectAccessible(page);
});

test("more Skills can be used at once", async ({ page }) => {
  await open(page);
  await startSession(page);
  const picker = page.getByRole("combobox", { name: "Skill" });
  await picker.selectOption("teacher");
  await page.getByRole("button", { name: /More Skills/ }).click();
  const dialog = page.getByRole("dialog", { name: "Also use" });
  await dialog.getByRole("checkbox", { name: "Technical Architect" }).check();
  await expect(page.getByRole("button", { name: /More Skills/ })).toHaveText(/\+1/);
  await expectAccessible(page);

  // The combined Skills answer: both names are in the system prompt, and the
  // architect's calculator is available.
  await page.keyboard.press("Escape");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("textbox", { name: "Ask a question" }).fill("Is 12 * 50 over budget?");
  await page.keyboard.press("Enter");
  await expect(
    page
      .getByRole("article", { name: "Chat: Is 12 * 50 over budget?" })
      .getByRole("list", { name: "Tools used" }),
  ).toHaveText("Calculator: 12 * 50 = 600");
});
