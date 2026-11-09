import { endIfLive, selectValue, waitForText } from "../helpers";

/**
 * T-048 against the real app: a theme picked in the Settings window reaches
 * the overlay through Rust's settings-changed event, and click-through goes
 * through the real window command and event. (Whether the OS really passes
 * clicks through is a manual check.)
 */
type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

const theme = () => browser.execute(() => document.documentElement.dataset.theme ?? "");

async function pickTheme(value: string) {
  await $("button=Settings").click();
  await browser.tauri.switchWindow("settings");
  await selectValue("theme", value);
  await browser.tauri.switchWindow("overlay");
}

describe("themes and click-through (PRD UI-8, UI-5)", () => {
  it("a Settings theme reaches the overlay", async () => {
    await endIfLive();
    await pickTheme("light");
    await browser.waitUntil(async () => (await theme()) === "light", {
      timeoutMsg: "the overlay did not switch to the light theme",
    });
    await pickTheme("high_contrast");
    await browser.waitUntil(async () => (await theme()) === "high-contrast", {
      timeoutMsg: "the overlay did not switch to high contrast",
    });
    await pickTheme("dark");
    await browser.waitUntil(async () => (await theme()) === "dark");
  });

  it("click-through turns on from the View menu and off from the app", async () => {
    await $("button=View").click();
    await $('input[role="switch"][aria-label="Click-through"]').click();
    const notice = await $('//p[@role="status"][contains(., "Click-through is on")]');
    await notice.waitForExist();
    await waitForText(notice, "turns it off");
    const on = await browser.execute(async () => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      return invoke("get_click_through");
    });
    if (on !== true) throw new Error("click-through not on in the app");

    // What the shortcut does: the same command; the event updates the overlay.
    await browser.execute(async () => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      await invoke("set_click_through", { on: false });
    });
    await notice.waitForExist({ reverse: true });
  });
});
