import { createAndStart, endIfLive, waitForLine, waitForText } from "../helpers";

/**
 * T-047 against the real app: vocabulary saved in Settings reaches the
 * speech-to-text provider when a session starts. The mock provider spells a
 * keyterm the way the vocabulary does ("Pro tier" in the fixture becomes
 * "PRO Tier"), which makes the boost visible.
 */
async function saveVocabulary(text: string) {
  await $("button=Settings").click();
  await browser.tauri.switchWindow("settings");
  const section = await $('//section[.//h2[normalize-space()="Custom vocabulary"]]');
  const box = await section.$("aria/Vocabulary terms");
  await box.waitForDisplayed();
  await box.clearValue();
  if (text) await box.setValue(text);
  await section.$("button=Save vocabulary").click();
  await waitForText(section, text ? "Saved" : "Vocabulary cleared.");
  await browser.tauri.switchWindow("overlay");
}

describe("custom vocabulary (PRD LIS-5)", () => {
  it("is sent to speech-to-text when a session starts", async () => {
    await endIfLive();
    await saveVocabulary("PRO Tier\nKubernetes");
    await createAndStart("Vocabulary check", "Transcribe me only");
    await waitForLine("What's the rate limit for the PRO Tier?");
    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
    // Later specs expect the plain fixture text.
    await saveVocabulary("");
  });
});
