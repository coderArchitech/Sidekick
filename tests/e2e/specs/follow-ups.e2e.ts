import { createAndStart, openTab, waitForLine, waitForText } from "../helpers";

/**
 * T-042 against the real app: a follow-up chip on a detected answer asks
 * through the Rust gateway (mock model) and the answer shows in Chat.
 */
describe("follow-up chips (PRD QA-9)", () => {
  it("Shorter on a live answer answers in Chat", async () => {
    await createAndStart("Follow-ups", "Participants know and agree to transcription");
    await waitForLine("What's the rate limit for the Pro tier?");

    await openTab("Live Answers");
    const card = await $("aria/Answer to: What's the rate limit for the Pro tier?");
    await card.waitForDisplayed({ timeout: 20_000 });
    const shorter = await card.$("button=Shorter");
    await shorter.waitForDisplayed({ timeout: 20_000 });
    await shorter.click();

    const turn = await $("aria/Chat: Shorter: What's the rate limit for the Pro tier?");
    await turn.waitForDisplayed({ timeout: 20_000 });
    // The mock model echoes what it was sent.
    await waitForText(turn, "at most two short sentences", 20_000);
    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  });
});
