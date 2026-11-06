import { expect } from "@wdio/globals";
import { createAndStart, openTab, waitForLine } from "../helpers";

// The app runs with SIDEKICK_MOCK_LLM_FAIL=1 for this spec (wdio.conf.ts).
describe("LLM outage (NFR-5)", () => {
  it("shows the error on the answer and keeps the transcript running", async () => {
    await createAndStart("Outage", "Participants know and agree to transcription");
    await waitForLine("What's the rate limit for the Pro tier?");

    await openTab("Live Answers");
    const card = await $("aria/Answer to: What's the rate limit for the Pro tier?");
    await card.waitForDisplayed({ timeout: 20_000 });
    await expect(card.$('[role="alert"]')).toBeDisplayed();

    // Later lines still arrive while answers fail.
    await waitForLine("Thanks, that's all for today.");
    await expect($("button=Pause")).toBeDisplayed();
  });
});
