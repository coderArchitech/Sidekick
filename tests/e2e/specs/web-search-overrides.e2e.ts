import { endIfLive, openTab, selectValue, waitForText } from "../helpers";

/**
 * T-046 against the real app: a session created with its own providers keeps
 * them (stored by Rust), and an agent Skill's web search goes through the
 * gateway's web search (mock mode: one deterministic result).
 */
type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

describe("web search and per-session providers (PRD KEY-8, KEY-9)", () => {
  it("own providers, then a web-searching agent", async () => {
    await endIfLive();
    // Settings window: web search on (Tavily) and a Skill that may use it.
    await $("button=Settings").click();
    await browser.tauri.switchWindow("settings");
    await browser.execute(async () => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      await invoke("update_settings", { patch: { search: { provider: "tavily", model: null } } });
      await invoke("save_skill", {
        skill: {
          id: "market-watch",
          name: "Market Watch",
          description: "",
          triggers: ["question_detected", "chat", "screenshot"],
          tools: ["web_search"],
          answerFormat: "cue_then_full",
          knowledge: [],
          instructions: "Use current numbers.",
        },
      });
    });
    await browser.tauri.switchWindow("overlay");

    await $("button=Options").click();
    await selectValue("Answers for this session", "openai");
    // Providers that aren't available (or have no key) are listed but can't be picked.
    const stt = await $('[role="combobox"][aria-label="Speech-to-text for this session"]');
    await stt.click();
    const soniox = await $(
      `[id="${await stt.getAttribute("aria-controls")}"] [role="option"][data-value="soniox"]`,
    );
    await soniox.waitForExist();
    if ((await soniox.getAttribute("aria-disabled")) !== "true")
      throw new Error("Soniox should not be selectable in this build");
    await browser.keys("Escape");
    await $('input[placeholder="New session name"]').setValue("Market call");
    await $("button=New session").click();
    await $("button=Start").click();
    await $("aria/Transcribe me only").click();
    await $("button=Start listening").click();
    await $("button=Pause").waitForDisplayed();

    const stored = (await browser.execute(async () => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      const page = (await invoke("list_sessions", { cursor: null, limit: 1 })) as {
        items: { sttProvider: string; llmProvider: string }[];
      };
      return page.items[0];
    })) as { sttProvider: string; llmProvider: string };
    if (stored.sttProvider !== "deepgram" || stored.llmProvider !== "openai")
      throw new Error(`session providers not kept: ${JSON.stringify(stored)}`);

    await selectValue("Skill", "market-watch");
    await openTab("Chat");
    await $("aria/Ask a question").setValue("What is the latest Fed rate?");
    await browser.keys("Enter");
    const turn = await $("aria/Chat: What is the latest Fed rate?");
    await turn.waitForDisplayed({ timeout: 20_000 });
    const tools = await turn.$('ul[aria-label="Tools used"]');
    await tools.waitForExist({ timeout: 20_000 });
    await waitForText(tools, "Web search", 20_000);
    await waitForText(turn, "Mock result for What is the latest Fed rate?", 20_000);

    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  });
});
