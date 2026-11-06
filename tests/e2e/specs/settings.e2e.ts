import { waitForText } from "../helpers";

describe("settings and keys [smoke]", () => {
  it("adds, tests and removes a key (mock provider)", async () => {
    await $("button=Settings").click();
    await browser.tauri.switchWindow("settings");
    // Every provider section has the same buttons: work inside "Answers (LLM)".
    const llm = await $('//section[.//h2[normalize-space()="Answers (LLM)"]]');
    const key = await llm.$("aria/Anthropic (Claude) API key");
    await key.waitForDisplayed();
    await key.setValue("sk-ant-e2e-test-key-9876");
    await llm.$("button=Save and test").click();
    await waitForText(llm, "Key works and is saved.");
    await waitForText(llm, "Saved key ending in ••••9876");

    await llm.$("button=Test connection").click();
    await waitForText(llm, "Connection works.");

    await llm.$("button=Remove key").click();
    await waitForText(llm, "Key removed.");
    await browser.waitUntil(async () => !(await llm.getText()).includes("9876"), {
      timeoutMsg: "the key was not removed",
    });
    await browser.tauri.switchWindow("overlay");
  });
});
