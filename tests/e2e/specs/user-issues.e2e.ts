import {
  createAndStart,
  endIfLive,
  openTab,
  selectValue,
  waitForLine,
  waitForText,
} from "../helpers";

/**
 * The reported issues, against the real app (mock providers, fixture audio):
 * 1. the overlay stays on top, also after another window took focus;
 * 2. dropdowns open inside the (screen-share protected) window, no native popup;
 * 3. the Staff Engineer Interview (JS) Skill is built in;
 * 4. a screenshot taken on Live Answers is answered there as the question;
 * 5. the other person's task ("Design a URL shortener…") is detected and answered.
 */
type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

const onTop = () =>
  browser.execute(async () => {
    const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
      .__TAURI_INTERNALS__.invoke;
    return invoke("plugin:window|is_always_on_top", { label: "overlay" });
  });

describe("reported issues", () => {
  it("1: the overlay is always on top, also after another window had focus", async () => {
    await endIfLive();
    // Linux reports "above" only when a window manager confirms it, and the
    // Xvfb used for E2E has none: there, check only that the query works.
    // Windows and macOS (CI runs both) check the real state.
    const check = async (when: string) => {
      const top = await onTop();
      if (process.platform === "linux") {
        if (typeof top !== "boolean") throw new Error(`no always-on-top state ${when}`);
      } else if (top !== true) {
        throw new Error(`the overlay isn't always on top ${when}`);
      }
    };
    await check("at start");
    await $("button=Settings").click();
    await browser.tauri.switchWindow("settings");
    await browser.tauri.switchWindow("overlay");
    await check("after another window had focus");
  });

  it("2 + 3: the Skill dropdown opens in the window and offers the interview Skill", async () => {
    await createAndStart("Staff interview", "The other people and me");
    const nativeSelects = await browser.execute(() => document.querySelectorAll("select").length);
    if (nativeSelects !== 0) throw new Error(`${nativeSelects} native <select> popups remain`);
    const picker = await $('[role="combobox"][aria-label="Skill"]');
    await picker.click();
    const list = await $(`[id="${await picker.getAttribute("aria-controls")}"]`);
    await list.waitForDisplayed();
    await waitForText(list, "Staff Engineer Interview (JS)");
    await browser.keys("Escape");
    await selectValue("Skill", "staff-engineer-interview");
  });

  it("5: the other person's task is transcribed, detected and answered", async () => {
    await waitForLine("Design a URL shortener for our docs.", 40_000);
    await openTab("Live Answers");
    const card = await $("aria/Answer to: Design a URL shortener for our docs.");
    await card.waitForDisplayed({ timeout: 20_000 });
    // The answer uses the interview Skill (the mock model echoes its prompt).
    await waitForText(card, "Mock answer to:", 20_000);
  });

  it("4: a screenshot on Live Answers is answered there", async () => {
    await openTab("Live Answers");
    await $("button=Screenshot").click();
    await $("aria/Full screen").click();
    const card = await $("aria/Answer to: Question on screen");
    await card.waitForDisplayed({ timeout: 20_000 });
    await waitForText(card, "Mock answer (with screenshot) to:", 20_000);
    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  });
});
