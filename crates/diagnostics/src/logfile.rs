//! Log files (tech doc 10.3): JSON lines in `<dir>/sidekick-YYYY-MM-DD.log`,
//! a new file each UTC day (and `.1`, `.2`, … past 10 MB), files older than
//! 14 days deleted, and at most 50 MB in total (oldest deleted first).
//!
//! Every line passes through [`redact`] on its way to disk: logging code must
//! never log secrets (tech doc 7.7), and this is the last line of defence.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use regex::Regex;
use tracing_subscriber::fmt::MakeWriter;

pub const KEEP_DAYS: i64 = 14;
pub const MAX_TOTAL_BYTES: u64 = 50 * 1024 * 1024;
pub const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const PREFIX: &str = "sidekick-";

/// Key-shaped strings: provider key prefixes, auth headers, and long hex
/// (Deepgram keys, the database key). Matches become `<first 4>…[redacted]`.
static SECRET: LazyLock<Option<Regex>> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?i:bearer|token|basic)\s+[A-Za-z0-9._~+/=-]{12,}",
        r"|sk-(?:ant-|proj-)?[A-Za-z0-9_-]{16,}",
        r"|AIza[0-9A-Za-z_-]{20,}",
        r"|\b[0-9a-fA-F]{32,}\b",
    ))
    .ok()
});

/// Masks key-shaped strings in one log line.
#[must_use]
pub fn redact(line: &str) -> String {
    match SECRET.as_ref() {
        Some(re) => re
            .replace_all(line, |c: &regex::Captures<'_>| {
                let m = &c[0];
                let keep: String = m.chars().take(4).collect();
                format!("{keep}…[redacted]")
            })
            .into_owned(),
        None => line.to_owned(),
    }
}

/// Days since 1970-01-01 → (year, month, day), proleptic Gregorian (UTC).
#[must_use]
pub fn civil_date(days: i64) -> (i64, u32, u32) {
    // Howard Hinnant's days_from_civil inverse.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "1..=31 and 1..=12"
    )]
    (y, m as u32, d as u32)
}

fn day_name(days: i64) -> String {
    let (y, m, d) = civil_date(days);
    format!("{y:04}-{m:02}-{d:02}")
}

fn today() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs() / 86_400).unwrap_or(0))
}

/// `sidekick-2026-11-06.log` / `sidekick-2026-11-06.2.log` → (day string, part).
fn parse_name(name: &str) -> Option<(String, u32)> {
    let rest = name.strip_prefix(PREFIX)?.strip_suffix(".log")?;
    let (day, part) = match rest.split_once('.') {
        Some((d, p)) => (d, p.parse().ok()?),
        None => (rest, 0),
    };
    (day.len() == 10).then(|| (day.to_owned(), part))
}

/// Log files in `dir`, oldest first, with their sizes.
#[must_use]
pub fn list(dir: &Path) -> Vec<(PathBuf, u64)> {
    let mut files: Vec<((String, u32), PathBuf, u64)> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let key = parse_name(e.file_name().to_str()?)?;
            let size = e.metadata().ok()?.len();
            Some((key, e.path(), size))
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files.into_iter().map(|(_, p, s)| (p, s)).collect()
}

/// Deletes files older than `keep_days` before `today`, then the oldest
/// until the total fits `max_total` (never `current`).
pub fn prune(dir: &Path, today: i64, keep_days: i64, max_total: u64, current: Option<&Path>) {
    let oldest_kept = day_name(today - keep_days + 1);
    let mut files = list(dir);
    files.retain(|(p, _)| {
        let old = p
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(parse_name)
            .is_some_and(|(day, _)| day < oldest_kept);
        if old && Some(p.as_path()) != current {
            let _ = std::fs::remove_file(p);
            return false;
        }
        true
    });
    let mut total: u64 = files.iter().map(|(_, s)| s).sum();
    for (p, size) in &files {
        if total <= max_total {
            break;
        }
        if Some(p.as_path()) == current {
            continue;
        }
        if std::fs::remove_file(p).is_ok() {
            total -= size;
        }
    }
}

struct Current {
    day: i64,
    part: u32,
    file: File,
    path: PathBuf,
    size: u64,
}

/// The log file writer. Cheap to share; writes are serialised.
pub struct LogFiles {
    dir: PathBuf,
    max_file: u64,
    max_total: u64,
    clock: fn() -> i64,
    current: Mutex<Option<Current>>,
}

impl LogFiles {
    /// Logs into `dir` (created if missing), pruning old files now.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self::with_limits(dir, MAX_FILE_BYTES, MAX_TOTAL_BYTES, today)
    }

    /// For tests: small limits and a fake clock (days since the epoch).
    #[must_use]
    pub fn with_limits(
        dir: impl Into<PathBuf>,
        max_file: u64,
        max_total: u64,
        clock: fn() -> i64,
    ) -> Self {
        let dir = dir.into();
        let _ = std::fs::create_dir_all(&dir);
        prune(&dir, clock(), KEEP_DAYS, max_total, None);
        Self {
            dir,
            max_file,
            max_total,
            clock,
            current: Mutex::new(None),
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn open(&self, day: i64, mut part: u32) -> Option<Current> {
        loop {
            let name = if part == 0 {
                format!("{PREFIX}{}.log", day_name(day))
            } else {
                format!("{PREFIX}{}.{part}.log", day_name(day))
            };
            let path = self.dir.join(name);
            let size = std::fs::metadata(&path).map_or(0, |m| m.len());
            if size >= self.max_file {
                part += 1;
                continue;
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok()?;
            return Some(Current {
                day,
                part,
                file,
                path,
                size,
            });
        }
    }

    /// Writes one already-formatted line (redacted here).
    pub fn write_line(&self, line: &[u8]) {
        let text = redact(&String::from_utf8_lossy(line));
        let bytes = text.as_bytes();
        let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        let day = (self.clock)();
        let roll = match guard.as_ref() {
            None => Some(0),
            Some(c) if c.day != day => Some(0),
            Some(c) if c.size + bytes.len() as u64 > self.max_file && c.size > 0 => {
                Some(c.part + 1)
            }
            Some(_) => None,
        };
        if let Some(part) = roll {
            let start = if guard.as_ref().is_some_and(|c| c.day == day) {
                part
            } else {
                0
            };
            *guard = self.open(day, start);
            if let Some(c) = guard.as_ref() {
                prune(&self.dir, day, KEEP_DAYS, self.max_total, Some(&c.path));
            }
        }
        if let Some(c) = guard.as_mut()
            && c.file.write_all(bytes).is_ok()
        {
            c.size += bytes.len() as u64;
        }
    }

    /// Flushes the current file (before exporting).
    pub fn flush(&self) {
        let mut guard = self.current.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(c) = guard.as_mut() {
            let _ = c.file.flush();
        }
    }
}

/// One formatted event, written as a single line when dropped.
pub struct LineWriter<'a> {
    files: &'a LogFiles,
    buf: Vec<u8>,
}

impl Write for LineWriter<'_> {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.buf.extend_from_slice(b);
        Ok(b.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for LineWriter<'_> {
    fn drop(&mut self) {
        if !self.buf.is_empty() {
            self.files.write_line(&self.buf);
        }
    }
}

impl<'a> MakeWriter<'a> for LogFiles {
    type Writer = LineWriter<'a>;
    fn make_writer(&'a self) -> Self::Writer {
        LineWriter {
            files: self,
            buf: Vec::with_capacity(256),
        }
    }
}

/// Writes a zip for a bug report: every log file plus `diagnostics.json`.
/// Nothing else (no database, no settings, no keys) goes in.
pub fn export_zip(log_dir: &Path, diagnostics_json: &str, out: &Path) -> std::io::Result<usize> {
    let file = File::create(out)?;
    let mut zip = zip::ZipWriter::new(file);
    let opts = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file("diagnostics.json", opts)
        .map_err(std::io::Error::other)?;
    zip.write_all(diagnostics_json.as_bytes())?;
    let logs = list(log_dir);
    for (path, _) in &logs {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        zip.start_file(format!("logs/{name}"), opts)
            .map_err(std::io::Error::other)?;
        zip.write_all(&std::fs::read(path)?)?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(logs.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn redacts_key_shaped_strings() {
        let line = r#"{"msg":"auth","h":"Bearer abcdefghijklmnop1234","k":"sk-ant-api03-AAAAAAAAAAAAAAAAAAAA","g":"AIzaSyA1234567890abcdefghij","dg":"0123456789abcdef0123456789abcdef01234567"}"#;
        let out = redact(line);
        for secret in [
            "abcdefghijklmnop1234",
            "AAAAAAAAAAAAAAAAAAAA",
            "1234567890abcdefghij",
            "0123456789abcdef0123456789abcdef",
        ] {
            assert!(!out.contains(secret), "{secret} leaked: {out}");
        }
        assert!(out.contains("sk-a…[redacted]"));
        // Ordinary content is untouched: ids, short hex, numbers.
        let ok = r#"{"request_id":"a1b2:cue","session_id":"0195f3a1-7c2e-7d41-9a3b-2f1c8e9d0a11","ms":1840}"#;
        assert_eq!(redact(ok), ok);
    }

    #[test]
    fn civil_dates() {
        assert_eq!(civil_date(0), (1970, 1, 1));
        assert_eq!(civil_date(20_398), (2025, 11, 6));
        assert_eq!(civil_date(11_016), (2000, 2, 29));
        assert_eq!(day_name(20_763), "2026-11-06");
    }

    static DAY: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(20_763);
    fn fake_today() -> i64 {
        DAY.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn names(dir: &Path) -> Vec<String> {
        list(dir)
            .into_iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn rotates_daily_and_by_size_and_keeps_14_days_within_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        DAY.store(20_763, std::sync::atomic::Ordering::SeqCst);
        // An old file from 20 days ago, and one from 10 days ago.
        std::fs::write(
            dir.path().join(format!("{PREFIX}{}.log", day_name(20_743))),
            "old\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join(format!("{PREFIX}{}.log", day_name(20_753))),
            "recent\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("unrelated.txt"), "keep").unwrap();
        let logs = LogFiles::with_limits(dir.path(), 100, 1_000, fake_today);
        assert_eq!(
            names(dir.path()),
            [format!("{PREFIX}2026-10-27.log")],
            "older than 14 days deleted at startup"
        );
        assert!(
            dir.path().join("unrelated.txt").exists(),
            "other files untouched"
        );

        let line = format!("{}\n", "x".repeat(39)); // 40 bytes
        for _ in 0..3 {
            logs.write_line(line.as_bytes());
        }
        assert_eq!(
            names(dir.path())[1..],
            [
                format!("{PREFIX}2026-11-06.log"),
                format!("{PREFIX}2026-11-06.1.log")
            ],
            "a new part past the per-file size"
        );
        DAY.store(20_764, std::sync::atomic::Ordering::SeqCst);
        logs.write_line(line.as_bytes());
        assert!(
            names(dir.path()).contains(&format!("{PREFIX}2026-11-07.log")),
            "a new file each day"
        );

        // Total cap: write enough to exceed 1,000 bytes; the oldest files go first.
        for _ in 0..40 {
            logs.write_line(line.as_bytes());
        }
        let total: u64 = list(dir.path()).iter().map(|(_, s)| s).sum();
        assert!(total <= 1_000 + 100, "total {total} stays near the cap");
        assert!(
            !names(dir.path()).contains(&format!("{PREFIX}2026-10-27.log")),
            "oldest deleted first"
        );
    }

    #[test]
    fn works_as_a_tracing_writer_and_redacts() {
        use tracing_subscriber::layer::SubscriberExt;
        let dir = tempfile::tempdir().unwrap();
        let logs = LogFiles::new(dir.path());
        let subscriber = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().json().with_writer(logs));
        tracing::subscriber::with_default(subscriber, || {
            tracing::info!(target: "gateway", request_id = "r1:full", "first token");
            tracing::warn!(target: "gateway", "bad header Bearer sk-ant-api03-SECRETSECRETSECRET");
        });
        let text: String = list(dir.path())
            .iter()
            .map(|(p, _)| std::fs::read_to_string(p).unwrap())
            .collect();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();
        assert_eq!(lines.len(), 2, "one JSON object per line");
        assert_eq!(lines[0]["fields"]["request_id"], "r1:full");
        assert!(!text.contains("SECRETSECRET"));
    }

    #[test]
    fn export_has_logs_and_diagnostics_only() {
        let dir = tempfile::tempdir().unwrap();
        let logs_dir = dir.path().join("logs");
        let logs = LogFiles::new(&logs_dir);
        logs.write_line(b"{\"message\":\"hello\"}\n");
        logs.flush();
        std::fs::write(dir.path().join("sidekick.db"), "database").unwrap();
        let out = dir.path().join("bundle.zip");
        assert_eq!(
            export_zip(&logs_dir, "{\"appVersion\":\"1\"}", &out).unwrap(),
            1
        );
        let mut zip = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let mut names: Vec<String> = (0..zip.len())
            .map(|i| zip.by_index(i).unwrap().name().to_owned())
            .collect();
        names.sort();
        assert_eq!(names.len(), 2);
        assert_eq!(names[0], "diagnostics.json");
        assert!(names[1].starts_with("logs/sidekick-") && names[1].ends_with(".log"));
        assert!(!names.iter().any(|n| n.contains(".db")));
    }
}
