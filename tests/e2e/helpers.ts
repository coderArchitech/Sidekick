/** Shared steps for the E2E specs. Selectors use accessible names (aria/…). */

/** The app is shared by the specs in a run: end a session left live by an earlier one. */
export async function endIfLive() {
  const end = await $("button=End");
  if (await end.isExisting()) {
    await end.click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  }
}

export async function createAndStart(
  title: string,
  consent: "Participants know and agree to transcription" | "Transcribe me only",
) {
  await endIfLive();
  await $('input[placeholder="New session name"]').setValue(title);
  await $("button=New session").click();
  await $("button=Start").click();
  await $(`aria/${consent}`).click();
  await $("button=Start listening").click();
  await $("button=Pause").waitForDisplayed();
}

export async function openTab(name: "Live Answers" | "Transcript" | "Notes" | "Chat") {
  await $(`button[role="tab"]=${name}`).click();
}

/** Waits for a transcript line containing `text`. */
export async function waitForLine(text: string, timeout = 30_000) {
  await openTab("Transcript");
  // The list only appears once there are lines, so look it up each time.
  await browser.waitUntil(
    async () => {
      const list = await $('ol[aria-label="Transcript"]');
      return (await list.isExisting()) && (await list.getText()).includes(text);
    },
    {
      timeout,
      timeoutMsg: `transcript never showed "${text}"`,
    },
  );
}

/**
 * Waits until `el`'s text contains `text`. (Partial-text selectors like
 * `*=…` resolve to elements WebKitGTK reports as not displayed.)
 */
export async function waitForText(
  el: { getText(): Promise<string> },
  text: string,
  timeout = 15_000,
) {
  await browser.waitUntil(async () => (await el.getText()).includes(text), {
    timeout,
    timeoutMsg: `never showed "${text}"`,
  });
}
