//! Developer tool for the T-010 manual check: streams live audio to Deepgram and
//! prints each final segment with its transcript lag (end of speech → segment
//! received), then p50/p95 (NFR-1 target: under 1 s / 1.5 s).
//!
//! The Deepgram key is read from the OS keychain, where the Settings window
//! saves it; keys are never taken from the command line or environment.
//!
//! ```text
//! cargo run -p sidekick-stt --example live_transcribe                 # microphone, 60 s
//! cargo run -p sidekick-stt --example live_transcribe -- --system     # system audio
//! cargo run -p sidekick-stt --example live_transcribe -- --seconds 30
//! cargo run -p sidekick-stt --example live_transcribe -- --fixture talk.wav
//! ```

#![allow(clippy::print_stdout, clippy::print_stderr)] // A command-line tool.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use sidekick_audio::{DeviceSource, FixtureSource, Pacing, StreamKind, frame_channel};
use sidekick_secrets::{KeyStore, KeychainStore};
use sidekick_stt::{
    DeepgramProvider, Partial, Segment, SttConfig, SttProvider, SttSink, SttStatus,
};
use tokio_util::sync::CancellationToken;

struct Printer {
    started: Instant,
    lags: Mutex<Vec<u32>>,
}

impl SttSink for Printer {
    fn partial(&self, p: Partial) {
        eprint!("\r  … {:<70.70}", p.text);
    }

    fn segment(&self, s: Segment) {
        let now_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let lag = u32::try_from(now_ms.saturating_sub(s.end_ms)).unwrap_or(u32::MAX);
        if let Ok(mut lags) = self.lags.lock() {
            lags.push(lag);
        }
        eprint!("\r{:80}\r", "");
        println!(
            "[{:>6} ms] {:<10} lag {:>5} ms  {}",
            s.end_ms, s.speaker_label, lag, s.text
        );
    }

    fn status(&self, kind: StreamKind, status: SttStatus) {
        eprintln!("\r({kind:?} stream: {status:?})");
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|a| a == flag)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let kind = if args.iter().any(|a| a == "--system") {
        StreamKind::System
    } else {
        StreamKind::Mic
    };
    let seconds: u64 = value("--seconds")
        .and_then(|s| s.parse().ok())
        .unwrap_or(60);

    let key = match KeychainStore::default().get("deepgram") {
        Ok(Some(key)) => key,
        Ok(None) => {
            eprintln!(
                "No Deepgram key found. Open Sidekick → Settings → Speech-to-text and save one first."
            );
            std::process::exit(2);
        }
        Err(e) => {
            eprintln!("Couldn't read the keychain: {}", e.message);
            std::process::exit(2);
        }
    };

    let (tx, rx) = frame_channel();
    let capture = match value("--fixture") {
        Some(path) => FixtureSource {
            path: path.into(),
            kind,
            pacing: Pacing::RealTime,
        }
        .start(tx),
        None => DeviceSource { kind }.start(tx),
    };
    let capture = match capture {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Couldn't start audio capture: {}", e.message);
            std::process::exit(3);
        }
    };

    let printer = Arc::new(Printer {
        started: Instant::now(),
        lags: Mutex::new(Vec::new()),
    });
    let cancel = CancellationToken::new();
    let sink: Arc<dyn SttSink> = printer.clone();
    let run_cancel = cancel.clone();
    let run = tokio::spawn(async move {
        DeepgramProvider::default()
            .run(SttConfig::new(kind), Some(key), rx, sink, run_cancel)
            .await
    });

    eprintln!("Listening to the {kind:?} stream for {seconds} s. Speak in full sentences…");
    tokio::time::sleep(Duration::from_secs(seconds)).await;
    capture.stop(); // Closes the frame channel; Deepgram flushes the last results.
    match tokio::time::timeout(Duration::from_secs(5), run).await {
        Ok(Ok(Err(e))) => eprintln!("Transcription stopped with an error: {}", e.message),
        Err(_) => cancel.cancel(),
        _ => {}
    }

    let mut lags = printer.lags.lock().map(|l| l.clone()).unwrap_or_default();
    lags.sort_unstable();
    let p50 = sidekick_diagnostics::percentile(&lags, 50);
    let p95 = sidekick_diagnostics::percentile(&lags, 95);
    println!(
        "\nSegments: {}  transcript lag p50: {:?} ms  p95: {:?} ms  (NFR-1 target: < 1000 / < 1500)",
        lags.len(),
        p50,
        p95
    );
    println!("Record these in docs/TECHNICAL.md section 13.2 with your OS and machine.");
}
