use std::path::PathBuf;

use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::config::AppConfig;
use crate::errors::AppResult;
use crate::rbac::guards::RequireSystemAdmin;

use super::{service, BackupActionResponse, BackupListResponse};

// ── Helper ────────────────────────────────────────────────────────────────────

fn backup_dir(config: &AppConfig) -> PathBuf {
    PathBuf::from(&config.backup_dir)
}

// ── Endpoints ─────────────────────────────────────────────────────────────────

/// List all locally retained backup files, newest first.
///
/// Returns the filename, size in bytes, and ISO 8601 creation timestamp for
/// each backup.  Also echoes the backup directory and retention policy.
///
/// **Required permission:** `system:admin`
#[get("/admin/backups")]
pub async fn list_backups(
    _guard: RequireSystemAdmin,
    config: &State<AppConfig>,
) -> AppResult<Json<BackupListResponse>> {
    let dir = backup_dir(config);
    Ok(Json(service::list_backups(
        &dir,
        config.backup_retain_days,
    )?))
}

/// Trigger an immediate backup outside the nightly schedule.
///
/// Runs `VACUUM INTO` on the live database to create a clean, consistent
/// snapshot.  Old backups are rotated according to the configured retention
/// policy.
///
/// **Required permission:** `system:admin`
#[post("/admin/backups")]
pub async fn trigger_backup(
    _guard: RequireSystemAdmin,
    config: &State<AppConfig>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<BackupActionResponse>> {
    let dir = backup_dir(config);
    let entry = service::create_backup(conn.inner(), &dir, config.backup_retain_days).await?;
    Ok(Json(BackupActionResponse {
        status: "ok",
        message: format!("Backup '{}' created successfully.", entry.filename),
        backup: Some(entry),
    }))
}

/// Stage a restore from the named backup file.
///
/// The backup is validated (SQLite magic bytes), then the target filename is
/// written to a `.restore_pending` marker in the backup directory.  **The
/// restore is applied on the next server restart** — the running process
/// continues to serve traffic normally.
///
/// To complete the restore: restart the server.  On startup the application
/// detects the marker, copies the backup over the live database file, removes
/// the marker, and proceeds with a clean migration run.
///
/// **Required permission:** `system:admin`
#[post("/admin/backups/<filename>/restore")]
pub async fn restore_backup(
    _guard: RequireSystemAdmin,
    filename: &str,
    config: &State<AppConfig>,
) -> AppResult<Json<BackupActionResponse>> {
    let dir = backup_dir(config);
    let staged_path = service::stage_restore(&dir, filename)?;
    Ok(Json(BackupActionResponse {
        status: "pending_restart",
        message: format!(
            "Restore of '{}' staged at '{}'. Restart the server to apply.",
            filename,
            staged_path.display()
        ),
        backup: None,
    }))
}
