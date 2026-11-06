//! Logging setup (tech doc 7.7, 10.3). JSON lines to stderr and to rotating
//! files in the app's log folder (or `<SIDEKICK_DATA_DIR>/logs`), every line
//! redacted for key-shaped strings. Level from `SIDEKICK_LOG` (default `info`).
//! Never log keys, prompts, transcript text, document content or answers.

use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

use sidekick_diagnostics::logfile::LogFiles;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

static FILES: OnceLock<Arc<LogFiles>> = OnceLock::new();

/// Shares one `LogFiles` with the tracing layer.
#[derive(Clone)]
struct Shared(Arc<LogFiles>);

impl<'a> MakeWriter<'a> for Shared {
    type Writer = sidekick_diagnostics::logfile::LineWriter<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        self.0.make_writer()
    }
}

/// Where logs go: `<SIDEKICK_DATA_DIR>/logs` in tests and demos, else the OS log folder.
pub fn log_dir(os_log_dir: PathBuf) -> PathBuf {
    std::env::var_os("SIDEKICK_DATA_DIR").map_or(os_log_dir, |d| PathBuf::from(d).join("logs"))
}

/// Call once, as early as the log folder is known.
pub fn init(dir: PathBuf) {
    let files = Arc::new(LogFiles::new(dir));
    let _ = FILES.set(Arc::clone(&files));
    let filter =
        || EnvFilter::try_from_env("SIDEKICK_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_filter(filter()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .json()
                .with_ansi(false)
                .with_writer(Shared(files))
                .with_filter(filter()),
        )
        .try_init();
}

/// The log files, for export.
pub fn files() -> Option<Arc<LogFiles>> {
    FILES.get().cloned()
}
