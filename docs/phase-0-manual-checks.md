# Phase 0 manual checks

Phase 0 code and automated tests are complete and green in CI on Linux, Windows and macOS. These checks need real hardware and can't run in CI. Each one closes the open box of a task in [Task.md](Task.md). Do them on a Windows 10/11 laptop and a Mac on macOS 14.6 or later.

Record results by ticking the box in Task.md and adding a note (OS version, meeting app, result). When all boxes for a task are ticked, set its status to ✅ Done.

## Setup (both machines)

Prerequisites are in [TECHNICAL.md 6.1](TECHNICAL.md#61-prerequisites).

```bash
git clone https://github.com/coderArchitech/Sidekick.git
```

```bash
cd Sidekick && pnpm install
```

## T-008: Settings window in the real app (5 minutes)

```bash
pnpm tauri dev
```

1. Click **Settings** in the overlay header. The Settings window opens.
2. Under **Answers (LLM)**, paste an Anthropic key and click **Save and test**. Expect "Key works and is saved." and "Saved key ending in ••••xxxx".
3. On macOS, allow the keychain prompt ("Always Allow").
4. Quit and restart the app. Open Settings: the key still shows as saved.
5. Click **Remove key**. Open Keychain Access (macOS) or Credential Manager (Windows): no `sidekick` entry for `anthropic`.

## T-009: Audio capture (10 minutes)

Play any audio (a video), then:

```bash
cargo test -p sidekick-audio -- --ignored
```

Both `real_microphone_produces_frames` and `real_system_audio_produces_frames` should pass. On macOS, grant Microphone and System Audio Recording when asked (System Settings → Privacy & Security), then run again.

Then, in a real Teams, Zoom or Meet call, run the same command while someone talks. The other participant hears no glitches.

Also check on macOS: deny System Audio Recording and run the system audio test. It should fail with a clear "can't record system audio" message, not hang.

## T-010: Deepgram latency (10 minutes)

1. Add a Deepgram key in Settings (it's checked against Deepgram before saving).
2. Until sessions exist (T-013), measure with the stt integration by pointing it at Deepgram: speak a sentence and note the time from the end of the sentence to the final segment. Target: under 1 s (NFR-1).
3. Record p50 and p95 over 10 sentences in [TECHNICAL.md 13.2](TECHNICAL.md#132-benchmarks-and-baselines).

## T-011: Screen-share hiding (20 minutes per OS)

```bash
pnpm tauri dev
```

For each of Teams, Zoom, Google Meet, Webex and Slack huddles:

1. Start a call with a second device (or a colleague) watching.
2. Share **the full screen**. On the other device, is the Sidekick overlay visible? (It should not be.)
3. Share **a single window** behind the overlay. Same question.
4. Note the shield colour in the overlay header (green, amber or red).

Record a table in [TECHNICAL.md 8.5](TECHNICAL.md#85-known-issues-and-limitations): OS version × meeting app × full screen / window → hidden or visible. If results contradict the shield (for example, green but visible), report it: the rules in `crates/window/src/lib.rs` get updated to match.

## T-012: Baselines (15 minutes)

On each reference laptop (8 GB Windows 11, 8 GB Apple Silicon MacBook Air):

1. **Cold start:** time from launching the built app to the overlay appearing.
2. **Installer size:** run `pnpm tauri build` and note the size of the `.msi` / `.dmg`.
3. **CPU and RAM while idle:** open Task Manager or Activity Monitor with the app running.

Record them in [TECHNICAL.md 13.2](TECHNICAL.md#132-benchmarks-and-baselines). End-to-end question → answer latency needs sessions (T-013) and moves there.
