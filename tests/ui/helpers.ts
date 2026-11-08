import AxeBuilder from "@axe-core/playwright";
import { expect, type Page } from "@playwright/test";

/** Opens the overlay (or the Settings window) with fresh mocks. */
export async function open(page: Page, which: "overlay" | "settings" = "overlay") {
  await page.goto(which === "settings" ? "/?window=settings" : "/");
  await page.waitForFunction(() => "__sidekickTest" in window);
}

/** Creates a session, confirms consent and starts listening. */
export async function startSession(
  page: Page,
  title = "Design review",
  consent:
    | "Participants know and agree to transcription"
    | "Transcribe me only" = "Participants know and agree to transcription",
) {
  await page.getByLabel("Session name").fill(title);
  await page.getByRole("button", { name: "New session" }).click();
  await page.getByRole("button", { name: "Start", exact: true }).click();
  const dialog = page.getByRole("dialog", { name: "Before Sidekick listens" });
  await dialog.getByLabel(consent).check();
  await dialog.getByRole("button", { name: "Start listening" }).click();
  await expect(page.getByRole("button", { name: "Pause" })).toBeVisible();
}

/** Adds a final transcript line from another speaker to the live session. */
export async function say(page: Page, text: string, speaker = "speaker_1") {
  await page.evaluate(
    async ({ text, speaker }) => {
      const t = (window as unknown as { __sidekickTest: SidekickTest }).__sidekickTest;
      const r = await t.commands.listSessions(null, null);
      const id = r.status === "ok" ? r.data.items[0]?.id : undefined;
      if (!id) throw new Error("no session");
      const lines = await t.commands.getTranscript(id);
      const end = lines.status === "ok" ? Math.max(0, ...lines.data.lines.map((l) => l.endMs)) : 0;
      t.transcriptLine(id, {
        speakerLabel: speaker,
        isUser: speaker === "me",
        text,
        startMs: end + 500,
        endMs: end + 2500,
      });
    },
    { text, speaker },
  );
}

/** Fails on any axe violation (WCAG 2.1 A/AA). */
export async function expectAccessible(page: Page, include?: string) {
  let builder = new AxeBuilder({ page }).withTags(["wcag2a", "wcag2aa", "wcag21a", "wcag21aa"]);
  if (include) builder = builder.include(include);
  const results = await builder.analyze();
  expect(
    results.violations.map(
      (v) => `${v.id}: ${v.help} (${v.nodes.map((n) => n.target.join(" ")).join(", ")})`,
    ),
  ).toEqual([]);
}

/** The shape of `window.__sidekickTest` (src/ipc/index.ts, uitest mode). */
interface SidekickTest {
  commands: {
    listSessions(
      c: null,
      l: null,
    ): Promise<{ status: "ok"; data: { items: { id: string }[] } } | { status: "error" }>;
    getTranscript(
      id: string,
    ): Promise<{ status: "ok"; data: { lines: { endMs: number }[] } } | { status: "error" }>;
    updateSettings(p: unknown): Promise<unknown>;
    createPack(name: string): Promise<{ status: "ok"; data: { id: string } }>;
    ingestPackDocument(packId: string, path: string): Promise<unknown>;
    saveSummary(
      id: string,
      summaryMd: string,
      items: { text: string; owner: string | null; due: string | null }[],
    ): Promise<unknown>;
  };
  transcriptLine(
    id: string,
    l: { speakerLabel: string; isUser: boolean; text: string; startMs: number; endMs: number },
  ): void;
  sessionBus: { emit(kind: string, e: unknown): void };
  files: { picked: string[]; saveAs: string[] };
}

export type { SidekickTest };
