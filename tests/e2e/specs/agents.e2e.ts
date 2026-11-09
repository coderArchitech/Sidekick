import { createAndStart, openTab, selectValue, waitForText } from "../helpers";

/**
 * T-045 against the real app: an agent Skill's tool step goes through the
 * Rust gateway (the mock model asks for the calculator on arithmetic), and
 * extra Skills are stored with the session.
 */
describe("agent Skills and several Skills (PRD SKL-4, SKL-5)", () => {
  it("calculator step, then extra Skills", async () => {
    await createAndStart("Deal review", "Transcribe me only");
    // The Skills list loads after the session starts; selectValue waits for it.
    await selectValue('select[aria-label="Skill"]', "negotiation-coach");

    await openTab("Chat");
    await $("aria/Ask a question").setValue("What is 2400 * 15% of the deal?");
    await browser.keys("Enter");
    const turn = await $("aria/Chat: What is 2400 * 15% of the deal?");
    await turn.waitForDisplayed({ timeout: 20_000 });
    const tools = await turn.$('ul[aria-label="Tools used"]');
    await tools.waitForExist({ timeout: 20_000 });
    await waitForText(tools, "2400 * 15% = 360", 20_000);

    const more = await $('//button[.//span[normalize-space()="More Skills"]]');
    await more.click();
    const dialog = await $('div[role="dialog"][aria-label="Also use"]');
    await dialog.$('//label[.//span[normalize-space()="Technical Architect"]]//input').click();
    await browser.waitUntil(async () => (await more.getText()).includes("+1"), {
      timeoutMsg: "the extra Skill was not saved",
    });
    await browser.keys("Escape");

    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  });
});
