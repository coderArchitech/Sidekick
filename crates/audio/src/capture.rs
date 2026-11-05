//! Running captures: pause, resume and stop (PRD LIS-6).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::AudioFrame;

/// Bounded so a stalled consumer drops audio instead of growing memory.
/// 200 frames = 10 s, matching the STT reconnect buffer (tech doc 5.4).
pub const CHANNEL_FRAMES: usize = 200;

pub type FrameSender = tokio::sync::mpsc::Sender<AudioFrame>;

/// A channel sized for audio frames.
#[must_use]
pub fn frame_channel() -> (FrameSender, tokio::sync::mpsc::Receiver<AudioFrame>) {
    tokio::sync::mpsc::channel(CHANNEL_FRAMES)
}

/// Controls a running capture. Dropping it stops the capture.
#[derive(Debug)]
pub struct CaptureHandle {
    paused: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CaptureHandle {
    pub(crate) fn new(
        paused: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
        thread: std::thread::JoinHandle<()>,
    ) -> Self {
        Self {
            paused,
            stopped,
            thread: Some(thread),
        }
    }

    /// Frames captured while paused are discarded, never buffered.
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Stops capturing and waits for the capture thread to finish.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stopped.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

impl Drop for CaptureHandle {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Shared flags handed to a capture thread.
#[derive(Debug, Clone, Default)]
pub(crate) struct Flags {
    pub paused: Arc<AtomicBool>,
    pub stopped: Arc<AtomicBool>,
}

impl Flags {
    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }
    pub fn stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }
}
