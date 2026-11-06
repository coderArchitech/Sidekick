import { expect } from "@wdio/globals";
import { createAndStart, openTab, waitForText } from "../helpers";

describe("screenshot [smoke]", () => {
  it("captures the screen and answers a question about it", async () => {
    await createAndStart("Bug triage", "Transcribe me only");
    await openTab("Chat");
    await $("button=Screenshot").click();
    await $("aria/Full screen").click();
    await waitForText(await $("aria/Screenshot attached"), "1568×882");

    const box = await $("aria/Ask a question");
    await box.setValue("What does this error mean?");
    await browser.keys("Enter");
    const turn = await $("aria/Chat: What does this error mean?");
    await browser.waitUntil(
      async () => (await turn.getText()).includes("Mock answer (with screenshot) to:"),
      {
        timeout: 20_000,
        timeoutMsg: "the screenshot never reached the model",
      },
    );
    await expect(turn.$("aria/Sources")).toHaveText(expect.stringContaining("Screenshot"));
  });
});
