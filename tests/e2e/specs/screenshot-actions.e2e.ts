import { createAndStart, openTab, waitForText } from "../helpers";

/**
 * T-043 against the real app: quick actions and the strip use the encrypted
 * screenshot store, and a re-ask sends that exact screenshot to the model.
 */
async function captureScreen() {
  await $("button=Screenshot").click();
  await $("aria/Full screen").click();
  await waitForText(await $("aria/Screenshot attached"), "1568×882");
}

describe("screenshot quick actions and strip (PRD SCR-5, SCR-6)", () => {
  it("quick action, strip, re-ask and delete", async () => {
    await createAndStart("Screen review", "Only me");
    await openTab("Chat");

    await captureScreen();
    await $("button=Summarise").click();
    const summary = await $("aria/Chat: Summarise (screenshot)");
    await summary.waitForDisplayed({ timeout: 20_000 });
    await waitForText(summary, "Mock answer (with screenshot) to:", 20_000);

    await captureScreen();
    await $("aria/Remove screenshot").click();
    const strip = await $("aria/Screenshots");
    await waitForText(strip, "Screenshots (2)");

    // Re-ask the first capture with a quick action.
    const asks = await strip.$$("button[aria-label^='Ask about the screenshot']");
    await asks[0]?.click();
    await $("aria/Screenshot attached").waitForDisplayed();
    await $("button=Explain").click();
    const explain = await $("aria/Chat: Explain (screenshot)");
    await explain.waitForDisplayed({ timeout: 20_000 });
    await waitForText(explain, "Mock answer (with screenshot) to:", 20_000);

    const deletes = await strip.$$("button[aria-label^='Delete the screenshot']");
    await deletes[0]?.click();
    await waitForText(strip, "Screenshots (1)");
  });
});
