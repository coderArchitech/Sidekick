# ADR-014: Audio capture with cpal; macOS system audio needs 14.6+

- **Status:** Accepted
- **Date:** 2026-11-05
- **Deciders:** @nitesh

## Context

ADR-006 chose OS-level capture. The first draft assumed ScreenCaptureKit for macOS system audio (macOS 13+, Screen Recording permission) and Silero VAD (an ONNX model) for voice activity detection.

cpal 0.18, the standard Rust audio library, now records system audio on both platforms by opening the output device for input: WASAPI loopback on Windows, and a Core Audio process tap on macOS 14.6 and later. Core Audio taps use the "System Audio Recording" permission (`NSAudioCaptureUsageDescription`), not Screen Recording.

## Decision

- Use cpal for the microphone and for system audio on Windows and macOS 14.6+.
- On macOS 13.0–14.5, system audio is not captured: the session runs in "Transcribe me only" mode and the UI says why. A ScreenCaptureKit path can be added later if users on those versions need it.
- Use a simple energy VAD with an adaptive noise floor in Phase 0, behind the same interface, and switch to Silero only if measurements show the need. The STT provider does its own endpointing; the VAD only marks silence.

## Alternatives considered

- **ScreenCaptureKit audio** (macOS 13+). Wider OS support, but a second code path with Objective-C bindings we can't compile or test outside a Mac, and it needs Screen Recording permission, which is a scarier prompt than audio-only.
- **Silero VAD now.** Better accuracy, but adds ONNX Runtime (several MB, native libraries per platform) to the installer before we know it's needed (NFR-11).

## Consequences

- One code path for all platforms; easy to test with WAV fixtures.
- macOS 13.0–14.5 users can only transcribe themselves until a ScreenCaptureKit path exists.
- The permission prompt and help text on macOS refer to System Audio Recording, not Screen Recording. Screenshots (T-025) still need Screen Recording.
- A perfectly steady sound longer than ~4 s is treated as background noise by the energy VAD; real speech has gaps, so this is acceptable.
