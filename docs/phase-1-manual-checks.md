# Phase 1 manual checks

These checks need real hardware or a real meeting and can't run in CI. Each one closes an open box of a task in [Task.md](Task.md). Setup is the same as in [Phase 0 manual checks](phase-0-manual-checks.md#setup-both-machines). Record results by ticking the box in Task.md with a note (OS version, meeting app, result).

## T-015: Shortcuts while a meeting app has focus (5 minutes per OS)

```bash
pnpm tauri dev
```

Start a session, click into Teams, Zoom or Meet so it has focus, then press each shortcut in [TECHNICAL.md 12.3](TECHNICAL.md#123-keyboard-shortcuts). Each one acts in Sidekick (pause, bookmark, Auto, Ask focuses Chat, Screenshot opens Chat with an attached screenshot) and the meeting app doesn't react.

## T-025: Screenshots during a live call (10 minutes per OS)

```bash
pnpm tauri dev
```

1. Start a session, then join a call in Teams, Zoom or Meet (share your screen if you can).
2. Press `Ctrl/Cmd+Shift+S`. Chat opens with "Screenshot attached". The meeting app shows no notification, and the call isn't interrupted.
3. Use **Screenshot → A window…** and pick the meeting window, then **An area…** and drag over part of the preview. Each attaches a screenshot of what you chose.
4. On macOS, the first capture asks for Screen Recording. Deny it once: Sidekick shows "Couldn't capture the screen…" with the System Settings path. Allow it and try again.
5. With an Anthropic key set, ask "What's on my screen?" after a capture. The answer describes the screen and cites "Screenshot hh:mm".

## T-027: Speech-to-text providers (30 minutes)

**AssemblyAI and Soniox.** In Settings → Speech-to-text, pick each provider, save its key (it's checked before saving), start a session and talk for a minute. Lines appear in the Transcript tab; with system audio, Soniox labels different speakers.

**Local Whisper.** Build with the feature, then download a model in Settings (Speech-to-text → Local Whisper → Download):

```bash
pnpm tauri dev -- --features local-whisper
```

Then run the accuracy test with a short English recording (16-bit WAV) and its words:

```bash
SIDEKICK_WHISPER_MODEL="$HOME/Library/Application Support/com.sidekick.app/models/ggml-tiny.en.bin" SIDEKICK_WHISPER_WAV=short-english.wav SIDEKICK_WHISPER_EXPECT="what is the rate limit for the pro tier" cargo test -p sidekick-stt --features local-whisper --test whisper -- --ignored
```

(On Windows the models folder is under `%APPDATA%\com.sidekick.app\models`.)

**Hinglish (PRD open question).** Play the same 2-minute Hinglish clip through Deepgram (language `multi`), Soniox and Whisper `base`/`small`, and record which reads best in [TECHNICAL.md 13.2](TECHNICAL.md#132-benchmarks-and-baselines).

## T-029: Overlay position on two monitors (10 minutes per OS)

With a second monitor connected, run `pnpm tauri dev`, drag the overlay onto the second monitor near its top-right corner and let go: it snaps flush to the edges. Quit and start again: it reopens there. Disconnect the second monitor and start again: it opens where it last was on the built-in display. Try A−/A+ and Compact, restart, and check they're kept.
