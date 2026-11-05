//! Integration tests for audio capture using generated WAV fixtures (Task T-009).

#![allow(
    clippy::unwrap_used,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]

use std::path::Path;
use std::time::Duration;

use sidekick_audio::{
    DeviceSource, FRAME_SAMPLES, FixtureSource, Pacing, StreamKind, frame_channel,
};

/// Writes `pattern` as alternating 44.1 kHz stereo segments: true = 300 Hz "voice", false = silence.
fn write_wav(path: &Path, pattern: &[(bool, f32)]) {
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: 44_100,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut w = hound::WavWriter::create(path, spec).unwrap();
    let mut t = 0usize;
    for &(voice, secs) in pattern {
        for _ in 0..(44_100.0 * secs) as usize {
            let v = if voice {
                (0.4 * (2.0 * std::f32::consts::PI * 300.0 * t as f32 / 44_100.0).sin() * 32_767.0)
                    as i16
            } else {
                0
            };
            w.write_sample(v).unwrap();
            w.write_sample(v).unwrap();
            t += 1;
        }
    }
    w.finalize().unwrap();
}

async fn collect(
    mut rx: tokio::sync::mpsc::Receiver<sidekick_audio::AudioFrame>,
) -> Vec<sidekick_audio::AudioFrame> {
    let mut frames = Vec::new();
    while let Ok(Some(f)) = tokio::time::timeout(Duration::from_secs(5), rx.recv()).await {
        frames.push(f);
    }
    frames
}

#[tokio::test]
async fn fixture_streams_16k_frames_with_speech_flags() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("meeting.wav");
    write_wav(&wav, &[(false, 1.0), (true, 1.0), (false, 1.0)]);
    let (tx, rx) = frame_channel();
    let handle = FixtureSource {
        path: wav,
        kind: StreamKind::System,
        pacing: Pacing::Fast,
    }
    .start(tx)
    .unwrap();
    let frames = collect(rx).await;
    handle.stop();

    assert!(
        (58..=60).contains(&frames.len()),
        "3 s = 60 frames, got {}",
        frames.len()
    );
    assert!(
        frames
            .iter()
            .all(|f| f.samples.len() == FRAME_SAMPLES && f.kind == StreamKind::System)
    );
    let speech: Vec<bool> = frames.iter().map(|f| f.speech).collect();
    assert!(speech[..19].iter().all(|s| !s), "leading silence");
    assert!(speech[21..39].iter().all(|s| *s), "voice section");
    assert!(
        speech[48..].iter().all(|s| !s),
        "trailing silence after hangover"
    );
    assert_eq!(frames[1].start_ms - frames[0].start_ms, 50);
}

#[tokio::test]
async fn pause_drops_frames_and_resume_continues() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("short.wav");
    write_wav(&wav, &[(true, 1.5)]);
    let (tx, rx) = frame_channel();
    let handle = FixtureSource {
        path: wav,
        kind: StreamKind::Mic,
        pacing: Pacing::RealTime,
    }
    .start(tx)
    .unwrap();
    tokio::time::sleep(Duration::from_millis(300)).await;
    handle.pause();
    assert!(handle.is_paused());
    tokio::time::sleep(Duration::from_millis(500)).await;
    handle.resume();
    let frames = collect(rx).await;
    handle.stop();
    // Robust to slow runners: the timestamps must show a gap (frames dropped
    // while paused) with frames on both sides of it (resume continues).
    let starts: Vec<u64> = frames.iter().map(|f| f.start_ms).collect();
    let gap = starts.windows(2).position(|w| w[1] - w[0] > 50);
    let Some(i) = gap else {
        panic!("no frames were dropped while paused: {starts:?}")
    };
    assert!(i >= 1, "frames before the pause: {starts:?}");
    assert!(i + 2 < starts.len(), "frames after resume: {starts:?}");
    assert!(
        frames.len() < 30,
        "not every frame delivered: {}",
        frames.len()
    );
}

#[tokio::test]
async fn stop_ends_the_stream() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("long.wav");
    write_wav(&wav, &[(true, 10.0)]);
    let (tx, mut rx) = frame_channel();
    let handle = FixtureSource {
        path: wav,
        kind: StreamKind::Mic,
        pacing: Pacing::RealTime,
    }
    .start(tx)
    .unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await;
    handle.stop();
    // Drain what was sent; the channel must then close.
    while rx.recv().await.is_some() {}
}

#[tokio::test]
async fn capture_writes_nothing_to_disk() {
    let dir = tempfile::tempdir().unwrap();
    let wav = dir.path().join("in.wav");
    write_wav(&wav, &[(true, 1.0)]);
    let before: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    let (tx, rx) = frame_channel();
    let handle = FixtureSource {
        path: wav,
        kind: StreamKind::Mic,
        pacing: Pacing::Fast,
    }
    .start(tx)
    .unwrap();
    let _ = collect(rx).await;
    handle.stop();
    let after: Vec<_> = std::fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(before, after, "NFR-6: no audio files written");
}

#[tokio::test]
async fn missing_fixture_is_a_clear_error() {
    let (tx, _rx) = frame_channel();
    let err = FixtureSource {
        path: "/nope.wav".into(),
        kind: StreamKind::Mic,
        pacing: Pacing::Fast,
    }
    .start(tx)
    .unwrap_err();
    assert_eq!(err.code, sidekick_core::ErrorCode::Internal);
}

#[test]
fn system_audio_is_unsupported_on_linux() {
    if cfg!(target_os = "linux") {
        let (tx, _rx) = frame_channel();
        let err = DeviceSource {
            kind: StreamKind::System,
        }
        .start(tx)
        .unwrap_err();
        assert_eq!(err.code, sidekick_core::ErrorCode::PermissionDenied);
    }
}

/// Needs a real microphone; run manually with `--ignored`.
#[tokio::test]
#[ignore = "needs a microphone"]
async fn real_microphone_produces_frames() {
    let (tx, mut rx) = frame_channel();
    let handle = DeviceSource {
        kind: StreamKind::Mic,
    }
    .start(tx)
    .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame.samples.len(), FRAME_SAMPLES);
    handle.stop();
}

/// Needs Windows or macOS 14.6+ with audio playing; run manually with `--ignored`.
#[tokio::test]
#[ignore = "needs system audio permission"]
async fn real_system_audio_produces_frames() {
    let (tx, mut rx) = frame_channel();
    let handle = DeviceSource {
        kind: StreamKind::System,
    }
    .start(tx)
    .unwrap();
    let frame = tokio::time::timeout(Duration::from_secs(3), rx.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(frame.kind, StreamKind::System);
    handle.stop();
}
