import { expect, test } from "@playwright/test";
import { expectAccessible, open, type SidekickTest, choose } from "./helpers";

/** T-046: web search for agent Skills (PRD KEY-8) and per-session providers (KEY-9). */
test("a session with its own providers; an agent searches the web", async ({ page }) => {
  await open(page);
  // Web search on (Settings → Web search), and a Skill that may use it.
  await page.evaluate(async () => {
    const t = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    await t.commands.updateSettings({ search: { provider: "tavily", model: null } });
    await t.commands.saveSkill({
      id: "market-watch",
      name: "Market Watch",
      description: "",
      triggers: ["question_detected", "chat", "screenshot"],
      tools: ["web_search"],
      answerFormat: "cue_then_full",
      knowledge: [],
      instructions: "Use current numbers.",
    });
  });

  await page.getByRole("button", { name: "Options" }).click();
  await choose(page.getByRole("combobox", { name: "Answers for this session" }), "openai");
  await choose(page.getByRole("combobox", { name: "Speech-to-text for this session" }), "soniox");
  await expectAccessible(page);
  await page.getByLabel("Session name").fill("Market call");
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await dialog.getByLabel("Transcribe me only").check();
  await dialog.getByRole("button", { name: "Start listening" }).click();

  const stored = await page.evaluate(async () => {
    const t = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
    const r = await t.commands.listSessions(null, null);
    return r.status === "ok" ? (r.data.items[0] as unknown as Record<string, string>) : null;
  });
  expect(stored).toMatchObject({ sttProvider: "soniox", llmProvider: "openai" });

  await choose(page.getByRole("combobox", { name: "Skill" }), "market-watch");
  await page.getByRole("tab", { name: "Chat" }).click();
  await page.getByRole("textbox", { name: "Ask a question" }).fill("What is the latest Fed rate?");
  await page.keyboard.press("Enter");
  const turn = page.getByRole("article", { name: "Chat: What is the latest Fed rate?" });
  await expect(turn.getByRole("list", { name: "Tools used" })).toHaveText(
    "Web search: What is the latest Fed rate?",
  );
  // The mock model echoes the prompt: the web result is in the answer's context.
  await expect(turn).toContainText("Mock result for What is the latest Fed rate?");
});
