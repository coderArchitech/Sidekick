import { expect, test } from "@playwright/test";
import { expectAccessible, open } from "./helpers";

test.beforeEach(async ({ page }) => open(page, "settings"));

const section = (page: import("@playwright/test").Page, name: RegExp) =>
  page.getByRole("region", { name });

test("each provider kind has a dropdown with the defaults selected", async ({ page }) => {
  const provider = (name: RegExp) =>
    section(page, name).getByRole("combobox", { name: "Provider" });
  await expect(provider(/speech-to-text/i)).toHaveValue("deepgram");
  await expect(provider(/answers/i)).toHaveValue("anthropic");
  await expect(provider(/embeddings/i)).toHaveValue("openai");
  await expect(provider(/web search/i)).toHaveValue("__off__");
  await expect(provider(/answers/i).locator("option")).toHaveText([
    "Anthropic (Claude)",
    "OpenAI",
    "Google Gemini",
    "Ollama (local)",
  ]);
  await expectAccessible(page);
});

test("saving a key unlocks the model dropdown; switching provider resets the model", async ({
  page,
}) => {
  const llm = section(page, /answers/i);
  await llm.getByLabel("Anthropic (Claude) API key").fill("sk-ant-test-key-1234");
  await llm.getByRole("button", { name: "Save and test" }).click();
  await expect(llm.getByText(/Key works and is saved/)).toBeVisible();
  const model = llm.getByRole("combobox", { name: "Model" });
  await expect(model).toBeVisible();
  await model.selectOption({ index: 1 });
  await expect(model).not.toHaveValue("");

  await llm.getByRole("combobox", { name: "Provider" }).selectOption("ollama");
  await expect(llm.getByText("Runs on this computer. No key needed.")).toBeVisible();
});

test("local Whisper shows its models and downloads one", async ({ page }) => {
  const stt = section(page, /speech-to-text/i);
  await stt.getByRole("combobox", { name: "Provider" }).selectOption("whisper-local");
  await expect(stt.getByRole("combobox", { name: "Model" })).toHaveValue("base");
  await stt.getByRole("button", { name: "Download (142 MB)" }).click();
  await expect(stt.getByText("Ready. Audio stays on this computer.")).toBeVisible();
});

test("keys are masked until shown", async ({ page }) => {
  const input = section(page, /answers/i).getByLabel("Anthropic (Claude) API key");
  await expect(input).toHaveAttribute("type", "password");
  await section(page, /answers/i)
    .getByRole("button", { name: "Show" })
    .click();
  await expect(input).toHaveAttribute("type", "text");
});
