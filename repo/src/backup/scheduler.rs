use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::{Timelike, Utc};
use sea_orm::DatabaseConnection;

use super::service;

/// Spawn a background Tokio task that runs a nightly database backup.
///
/// The first backup fires at the next UTC midnight; subsequent backups fire
/// every 24 hours.  If a backup fails the error is logged and the scheduler
/// continues running — it will retry at the next scheduled interval.
///
/// The task is detached from the caller and runs for the lifetime of the
/// process.
pub fn start(conn: Arc<DatabaseConnection>, backup_dir: PathBuf, retain_days: u32) {
    tokio::spawn(async move {
        // Sleep until the next UTC midnight so backups occur at a consistent time.
        let initial_delay = seconds_until_midnight();
        tracing::info!(
            delay_secs = initial_delay,
            "backup.scheduler_started — first run at next UTC midnight"
        );
        tokio::time::sleep(Duration::from_secs(initial_delay)).await;

        loop {
            tracing::info!("backup.nightly_run_starting");
            match service::create_backup(&conn, &backup_dir, retain_days).await {
                Ok(entry) => {
                    tracing::info!(
                        filename   = %entry.filename,
                        size_bytes = entry.size_bytes,
                        "backup.nightly_run_completed"
                    );
                }
                Err(e) => {
                    tracing::error!(error = %e, "backup.nightly_run_failed");
                }
            }

            // Sleep exactly 24 hours before the next run.
            tokio::time::sleep(Duration::from_secs(86_400)).await;
        }
    });
}

/// Compute the number of seconds from now until the next UTC midnight.
///
/// Returns at least 1 second to avoid a zero-length sleep on the exact boundary.
fn seconds_until_midnight() -> u64 {
    let now = Utc::now();
    let secs_since_midnight =
        now.hour() as u64 * 3600 + now.minute() as u64 * 60 + now.second() as u64;
    let secs_in_day: u64 = 86_400;
    let remaining = secs_in_day.saturating_sub(secs_since_midnight);
    remaining.max(1)
}
