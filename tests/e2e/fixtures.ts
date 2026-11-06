import { mkdtempSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** What the mock speech-to-text "hears", one utterance per burst of audio. */
export const SCRIPT = [
  "Welcome everyone to the design review.",
  "What's the rate limit for the Pro tier?",
  "Let's also look at the dashboard.",
  "Thanks, that's all for today.",
];

/**
 * A 16 kHz mono WAV with one tone burst per script line (loud enough for the
 * VAD), separated by silence. Real speech isn't needed: the mock STT maps
 * each burst to the next script line.
 */
export function writeFixtureWav(
  path: string,
  bursts = SCRIPT.length,
  burstSec = 1.2,
  gapSec = 1.5,
): void {
  const rate = 16_000;
  const lead = Math.round(rate * 0.5);
  const burst = Math.round(rate * burstSec);
  const gap = Math.round(rate * gapSec);
  const total = lead + bursts * (burst + gap);
  const data = Buffer.alloc(total * 2);
  let i = lead;
  for (let b = 0; b < bursts; b++) {
    for (let k = 0; k < burst; k++, i++) {
      const s = Math.round(
        9000 *
          Math.sin((2 * Math.PI * 220 * k) / rate) *
          (0.6 + 0.4 * Math.sin((2 * Math.PI * 3 * k) / rate)),
      );
      data.writeInt16LE(s, i * 2);
    }
    i += gap;
  }
  const header = Buffer.alloc(44);
  header.write("RIFF", 0);
  header.writeUInt32LE(36 + data.length, 4);
  header.write("WAVE", 8);
  header.write("fmt ", 12);
  header.writeUInt32LE(16, 16);
  header.writeUInt16LE(1, 20); // PCM
  header.writeUInt16LE(1, 22); // mono
  header.writeUInt32LE(rate, 24);
  header.writeUInt32LE(rate * 2, 28);
  header.writeUInt16LE(2, 32);
  header.writeUInt16LE(16, 34);
  header.write("data", 36);
  header.writeUInt32LE(data.length, 40);
  writeFileSync(path, Buffer.concat([header, data]));
}

/** A fresh data folder with the fixture audio and script; returns the env for the app. */
export function prepareRun(
  root: string,
  opts: { llmFails?: boolean } = {},
): Record<string, string> {
  const dir = mkdtempSync(join(root, "run-"));
  const wav = join(dir, "meeting.wav");
  const script = join(dir, "script.txt");
  writeFixtureWav(wav);
  writeFileSync(script, `${SCRIPT.join("\n")}\n`);
  return {
    SIDEKICK_DATA_DIR: join(dir, "data"),
    SIDEKICK_MOCK_PROVIDERS: "1",
    SIDEKICK_AUDIO_FIXTURE: wav,
    SIDEKICK_MOCK_TRANSCRIPT: script,
    SIDEKICK_MOCK_LLM_FAIL: opts.llmFails ? "1" : "0",
    SIDEKICK_LOG: "info",
  };
}
