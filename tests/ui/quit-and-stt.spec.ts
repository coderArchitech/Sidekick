import { expect, test, type Page } from "@playwright/test";
import { expectAccessible, open, startSession } from "./helpers";

/** The red quit button in the header, and speech-to-text problems in the session bar. */

type Hook = {
  commands: { listSessions(c: null, l: null): Promise<{ data: { items: { id: string }[] } }> };
  sessionBus: { emit(kind: string, e: unknown): void };
};

async function sttEvent(page: Page, stream: "mic" | "system", state: string) {
  await page.evaluate(
    async ([stream, state]) => {
      const t = (window as unknown as { __sidekickTest: Hook }).__sidekickTest;
      const id = (await t.commands.listSessions(null, null)).data.items[0]?.id;
      t.sessionBus.emit("stt", { sessionId: id, stream, state });
    },
    [stream, state],
  );
}

test("the header has a red quit button that passes axe", async ({ page }) => {
  await open(page);
  const quit = page.getByRole("banner").getByRole("button", { name: "Quit Sidekick" });
  await expect(quit).toBeVisible();
  const bg = await quit.evaluate((el) => getComputedStyle(el).backgroundColor);
  const [r, g, b] = (bg.match(/\d+/g) ?? []).map(Number);
  expect(r).toBeGreaterThan(150);
  expect(r).toBeGreaterThan((g ?? 0) * 2);
  expect(r).toBeGreaterThan((b ?? 0) * 2);
  await expectAccessible(page);
});

test("quitting during a session asks, then ends the session first", async ({ page }) => {
  await open(page);
  await startSession(page, "Design review");
  await page.getByRole("button", { name: "Quit Sidekick" }).click();
  const dialog = page.getByRole("dialog", { name: "Quit Sidekick?" });
  await expect(dialog).toContainText("“Design review” is still listening");
  await expectAccessible(page);
  await page.keyboard.press("Escape");
  await expect(dialog).toBeHidden();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();

  await page.getByRole("button", { name: "Quit Sidekick" }).click();
  await dialog.getByRole("button", { name: "End session and quit" }).click();
  await expect(page.getByText("Session ended.")).toBeVisible();
});

test("a silent microphone is called out with what to do, and clears when sound returns", async ({
  page,
}) => {
  await open(page);
  await startSession(page);
  await sttEvent(page, "mic", "silent");
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("isn't hearing your microphone");
  await expect(alert).toContainText("Microphone");
  await expectAccessible(page);
  await sttEvent(page, "mic", "live");
  await expect(page.getByRole("alert")).toHaveCount(0);
});

test("a failed stream says which one stopped", async ({ page }) => {
  await open(page);
  await startSession(page);
  await sttEvent(page, "system", "failed");
  await expect(page.getByRole("alert")).toContainText(
    "Speech-to-text stopped for other participants",
  );
});
