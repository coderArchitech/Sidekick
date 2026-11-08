import { expect } from "@wdio/globals";
import { createAndStart, openTab, waitForLine, waitForText } from "../helpers";

/**
 * T-041 against the real app (mock providers, fixture audio): the agenda is
 * stored in the encrypted database, moved on by the notes job and closed when
 * the session ends; "What did I miss?" recaps the recent transcript.
 */
describe("meeting memory (PRD MEM-2, MEM-3)", () => {
  it("agenda, recap and end-of-meeting agenda status", async () => {
    await createAndStart("Agenda review", "Participants know and agree to transcription");

    await openTab("Notes");
    await $("button=Add agenda").click();
    await $("aria/Agenda items, one per line").setValue("Pro tier pricing\nDashboard\nHiring");
    await $("button=Save agenda").click();
    const agenda = await $("aria/Agenda");
    await waitForText(agenda, "0/3 covered");

    // The fixture script mentions the Pro tier, then the dashboard.
    await waitForLine("Let's also look at the dashboard.");
    await openTab("Notes");
    await $("aria/Recap period").selectByAttribute("value", "10");
    await $("button=What did I miss?").click();
    const card = await $("aria/What you missed");
    await card.waitForDisplayed({ timeout: 20_000 });
    // The mock model echoes the transcript it was sent.
    await waitForText(card, "rate limit for the Pro tier");

    await $("button=End").click();
    await waitForText(agenda, "2/3 covered", 30_000);
    await expect($("aria/Status of Pro tier pricing")).toHaveValue("covered");
    await expect($("aria/Status of Hiring")).toHaveValue("not_started");
  });
});
