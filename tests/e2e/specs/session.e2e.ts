import { expect } from "@wdio/globals";
import { createAndStart, openTab, waitForLine, waitForText } from "../helpers";

describe("session [smoke]", () => {
  it("consent → fixture audio → transcript → detected question → cited answer", async () => {
    await createAndStart("Design review", "Participants know and agree to transcription");

    await waitForLine("What's the rate limit for the Pro tier?");

    await openTab("Live Answers");
    const card = await $("aria/Answer to: What's the rate limit for the Pro tier?");
    await card.waitForDisplayed({ timeout: 20_000 });
    await waitForText(card, "Mock answer to:");
    // The mock model cites source [1] in full answers (crates/gateway mock).
    const toggle = await card.$('.//button[contains(., "Full answer")]');
    await toggle.waitForExist();
    await toggle.click();
    await waitForText(await card.$("aria/Sources"), "[1] Transcript");

    await $("button=End").click();
    await expect($('input[placeholder="New session name"]')).toBeDisplayed();
  });
});
