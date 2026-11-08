import { writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createAndStart, endIfLive, openTab, waitForText } from "../helpers";

/**
 * T-044 against the real app: a Knowledge Pack made in Settings is ingested by
 * the Rust pipeline (mock embedder), turned on for a session in the overlay,
 * searched for a chat answer and read by the pre-meeting brief.
 */
type Invoke = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;

describe("Knowledge Packs and brief (PRD CTX-4, CTX-5)", () => {
  it("pack from Settings → attached → cited in Chat → in the brief", async () => {
    await endIfLive();
    await $("button=Settings").click();
    await browser.tauri.switchWindow("settings");
    const packs = await $('//section[.//h2[normalize-space()="Knowledge Packs"]]');
    await packs.$("aria/New pack name").setValue("E2E pack");
    await packs.$("button=Create pack").click();
    await waitForText(packs, "0/0 ready");

    // Native file dialogs can't be driven: queue the file through the same
    // command the "Add files" button uses.
    const file = join(tmpdir(), `sidekick-e2e-sla-${Date.now()}.md`);
    writeFileSync(
      file,
      "Enterprise customers get a 99.95 percent uptime SLA with service credits.\n",
    );
    await browser.execute(async (path: string) => {
      const invoke = (window as unknown as { __TAURI_INTERNALS__: { invoke: Invoke } })
        .__TAURI_INTERNALS__.invoke;
      const list = (await invoke("list_packs")) as { id: string; name: string }[];
      const pack = list.find((p) => p.name === "E2E pack");
      await invoke("ingest_pack_document", { packId: pack?.id, path });
    }, file);
    await waitForText(packs, "1/1 ready", 30_000);
    await browser.tauri.switchWindow("overlay");

    await createAndStart("Pack review", "Transcribe me only");
    await $("button*=Documents (").click();
    const box = await $('input[aria-label="Use E2E pack in this session"]');
    await box.waitForExist();
    await box.click();
    await browser.waitUntil(async () => box.isSelected(), { timeoutMsg: "pack not turned on" });

    await openTab("Chat");
    await $("aria/Ask a question").setValue("What uptime SLA do enterprise customers get?");
    await browser.keys("Enter");
    const turn = await $("aria/Chat: What uptime SLA do enterprise customers get?");
    await turn.waitForDisplayed({ timeout: 20_000 });
    await waitForText(turn, "sidekick-e2e-sla", 20_000);

    await openTab("Notes");
    const brief = await $('section[aria-label="Pre-meeting brief"]');
    await brief.$("button=Write brief").click();
    await waitForText(brief, "Mock answer to:", 20_000);

    await $("button=End").click();
    await $('input[placeholder="New session name"]').waitForDisplayed();
  });
});
