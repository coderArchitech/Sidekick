import { expect, test } from "@playwright/test";
import { expectAccessible, open, choose } from "./helpers";

test.beforeEach(async ({ page }) => open(page, "settings"));

const section = (page: import("@playwright/test").Page, name: RegExp) =>
  page.getByRole("region", { name });

test("each provider kind has a dropdown with the defaults selected", async ({ page }) => {
  const provider = (name: RegExp) =>
    section(page, name).getByRole("combobox", { name: "Provider" });
  await expect(provider(/speech-to-text/i)).toHaveAttribute("data-value", "deepgram");
  await expect(provider(/answers/i)).toHaveAttribute("data-value", "anthropic");
  await expect(provider(/embeddings/i)).toHaveAttribute("data-value", "openai");
  await expect(provider(/web search/i)).toHaveAttribute("data-value", "__off__");
  await provider(/answers/i).click();
  await expect(page.getByRole("listbox").getByRole("option")).toHaveText([
    "Anthropic (Claude)",
    "OpenAI",
    "Google Gemini",
    "Ollama (local)",
  ]);
  // The list is drawn inside the window (hidden in screen shares), and accessible.
  await expectAccessible(page);
  await page.keyboard.press("Escape");
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
  await choose(model, "mock-large");

  await choose(llm.getByRole("combobox", { name: "Provider" }), "ollama");
  await expect(llm.getByText("Runs on this computer. No key needed.")).toBeVisible();
});

test("local Whisper shows its models and downloads one", async ({ page }) => {
  const stt = section(page, /speech-to-text/i);
  await choose(stt.getByRole("combobox", { name: "Provider" }), "whisper-local");
  await expect(stt.getByRole("combobox", { name: "Model" })).toHaveAttribute("data-value", "base");
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
