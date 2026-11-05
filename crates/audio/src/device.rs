//! Live capture from audio devices via cpal.
//!
//! - Mic: the default input device.
//! - System: the default *output* device opened for input, which cpal turns into
//!   WASAPI loopback on Windows and a Core Audio process tap on macOS 14.6+
//!   (needs the "System Audio Recording" permission). Not supported on Linux.
//!
//! cpal streams aren't `Send` on every platform, so each capture owns a thread
//! that keeps the stream alive until stopped.

use std::sync::mpsc;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sidekick_core::{ErrorCode, SidekickError, SidekickResult};

use crate::capture::{CaptureHandle, Flags};
use crate::{FrameSender, Pipeline, StreamKind};

#[derive(Debug, Clone, Copy)]
pub struct DeviceSource {
    pub kind: StreamKind,
}

impl DeviceSource {
    /// Opens the device and starts sending frames. Fails fast (before returning)
    /// if the device is missing or permission is denied.
    pub fn start(&self, tx: FrameSender) -> SidekickResult<CaptureHandle> {
        let flags = Flags::default();
        let thread_flags = flags.clone();
        let kind = self.kind;
        let (ready_tx, ready_rx) = mpsc::channel::<SidekickResult<()>>();
        let thread = std::thread::Builder::new()
            .name(format!("sidekick-audio-{kind:?}").to_lowercase())
            .spawn(move || {
                let stream = match open_stream(kind, tx, thread_flags.clone()) {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                        return;
                    }
                };
                let _ = ready_tx.send(Ok(()));
                while !thread_flags.stopped() {
                    std::thread::sleep(Duration::from_millis(50));
                }
                drop(stream);
            })
            .map_err(|_| SidekickError::internal("Couldn't start audio capture."))?;
        let started = ready_rx
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| {
                Err(SidekickError::internal(
                    "The audio device didn't start in time.",
                ))
            });
        let handle = CaptureHandle::new(flags.paused, flags.stopped, thread);
        started.map(|()| handle)
    }
}

fn open_stream(kind: StreamKind, tx: FrameSender, flags: Flags) -> SidekickResult<cpal::Stream> {
    if kind == StreamKind::System && cfg!(target_os = "linux") {
        return Err(SidekickError::new(
            ErrorCode::PermissionDenied,
            "System audio capture isn't supported on Linux.",
        ));
    }
    let host = cpal::default_host();
    let device = match kind {
        StreamKind::Mic => host.default_input_device(),
        StreamKind::System => host.default_output_device(),
    }
    .ok_or_else(|| match kind {
        StreamKind::Mic => SidekickError::new(ErrorCode::PermissionDenied, "No microphone found."),
        StreamKind::System => {
            SidekickError::new(ErrorCode::PermissionDenied, "No audio output device found.")
        }
    })?;
    let supported = match kind {
        StreamKind::Mic => device.default_input_config(),
        StreamKind::System => device.default_output_config(),
    }
    .map_err(|e| device_error(kind, &e))?;

    let sample_format = supported.sample_format();
    let config: cpal::StreamConfig = supported.into();
    let mut pipeline = Pipeline::new(kind, config.sample_rate, config.channels);
    let on_error =
        move |e: cpal::Error| tracing::warn!(target: "audio", ?kind, error = %e, "stream error");

    // Converts any sample type to f32 and runs the pipeline. Never blocks: if
    // the consumer is behind, frames are dropped (audio is real-time).
    let mut handle = move |data: &[f32]| {
        if flags.paused() || flags.stopped() {
            return;
        }
        pipeline.push(data, |frame| {
            if tx.try_send(frame).is_err() {
                tracing::debug!(target: "audio", ?kind, "frame dropped");
            }
        });
    };
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            device.build_input_stream(config, move |d: &[f32], _: &_| handle(d), on_error, None)
        }
        cpal::SampleFormat::I16 => device.build_input_stream(
            config,
            move |d: &[i16], _: &_| {
                let f: Vec<f32> = d.iter().map(|&s| f32::from(s) / 32_768.0).collect();
                handle(&f);
            },
            on_error,
            None,
        ),
        cpal::SampleFormat::I32 => device.build_input_stream(
            config,
            move |d: &[i32], _: &_| {
                #[allow(clippy::cast_precision_loss)]
                let f: Vec<f32> = d.iter().map(|&s| s as f32 / 2_147_483_648.0).collect();
                handle(&f);
            },
            on_error,
            None,
        ),
        other => {
            return Err(SidekickError::internal(format!(
                "Unsupported audio format {other:?}."
            )));
        }
    }
    .map_err(|e| device_error(kind, &e))?;
    stream.play().map_err(|e| device_error(kind, &e))?;
    tracing::info!(target: "audio", ?kind, rate = config.sample_rate, channels = config.channels, "capture started");
    Ok(stream)
}

fn device_error(kind: StreamKind, err: &cpal::Error) -> SidekickError {
    tracing::warn!(target: "audio", ?kind, error = %err, "device error");
    let what = match kind {
        StreamKind::Mic => "the microphone",
        StreamKind::System => "system audio",
    };
    SidekickError::new(
        ErrorCode::PermissionDenied,
        format!("Sidekick can't record {what}. Check the device and the OS privacy settings."),
    )
}
