//! Scheduled jobs (tech doc 10.7): retention cleanup at start and every
//! 6 hours, and VACUUM after deletions once no session is live (idle).

use std::sync::atomic::Ordering;
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;

const RETENTION_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
const IDLE_CHECK_EVERY: Duration = Duration::from_secs(5 * 60);

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Runs the retention cleanup once; marks a VACUUM as needed if it deleted anything.
pub fn cleanup(state: &AppState) {
    match sidekick_session::retention::run(&state.db(), now_ms()) {
        Ok(r) if r.sessions_deleted > 0 || r.consent_records_deleted > 0 => {
            state.vacuum_pending.store(true, Ordering::SeqCst);
        }
        Ok(_) => {}
        Err(e) => tracing::warn!(target: "session", code = ?e.code, "retention cleanup failed"),
    }
}

/// VACUUMs if deletions are waiting and nothing is live (it rewrites the whole file).
pub async fn vacuum_if_idle(state: &AppState) {
    if !state.vacuum_pending.load(Ordering::SeqCst) || state.live.lock().await.is_some() {
        return;
    }
    let started = std::time::Instant::now();
    match state.db().vacuum() {
        Ok(()) => {
            state.vacuum_pending.store(false, Ordering::SeqCst);
            tracing::info!(target: "store", ms = started.elapsed().as_millis(), "vacuum done");
        }
        Err(e) => tracing::warn!(target: "store", code = ?e.code, "vacuum failed"),
    }
}

/// Starts the background jobs.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        cleanup(&state);
        let mut since_cleanup = Duration::ZERO;
        loop {
            vacuum_if_idle(&state).await;
            tokio::time::sleep(IDLE_CHECK_EVERY).await;
            since_cleanup += IDLE_CHECK_EVERY;
            if since_cleanup >= RETENTION_EVERY {
                since_cleanup = Duration::ZERO;
                cleanup(&state);
            }
        }
    });
}
