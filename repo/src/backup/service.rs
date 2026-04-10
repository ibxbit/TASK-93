use std::path::{Path, PathBuf};

use chrono::Utc;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};

use crate::errors::{AppError, AppResult};

use super::{BackupEntry, BackupListResponse};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Filename prefix for all backup files.
const BACKUP_PREFIX: &str = "backup_";
/// Extension used for SQLite backup files.
const BACKUP_EXT: &str = "sqlite";
/// Sentinel file written by the restore endpoint; read by the next startup.
const RESTORE_MARKER: &str = ".restore_pending";

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generate a timestamped backup filename, e.g. `backup_20260407_230000.sqlite`.
fn backup_filename() -> String {
    let ts = Utc::now().format("%Y%m%d_%H%M%S");
    format!("{BACKUP_PREFIX}{ts}.{BACKUP_EXT}")
}

/// Parse a `BackupEntry` from a filesystem path.  Returns `None` if the file
/// does not match the expected naming scheme.
fn entry_from_path(path: &Path) -> Option<BackupEntry> {
    let filename = path.file_name()?.to_str()?.to_owned();
    if !filename.starts_with(BACKUP_PREFIX) || !filename.ends_with(BACKUP_EXT) {
        return None;
    }

    let size_bytes = path.metadata().ok()?.len();

    // Extract the timestamp portion: `backup_YYYYMMDD_HHMMSS.sqlite`
    let ts_part = filename
        .strip_prefix(BACKUP_PREFIX)?
        .strip_suffix(&format!(".{BACKUP_EXT}"))?;

    // Convert compact timestamp to readable ISO 8601.
    // Format: YYYYMMDD_HHMMSS → YYYY-MM-DDTHH:MM:SSZ
    let created_at = if ts_part.len() == 15 {
        format!(
            "{}-{}-{}T{}:{}:{}Z",
            &ts_part[0..4],
            &ts_part[4..6],
            &ts_part[6..8],
            &ts_part[9..11],
            &ts_part[11..13],
            &ts_part[13..15],
        )
    } else {
        ts_part.to_owned()
    };

    Some(BackupEntry {
        filename,
        size_bytes,
        created_at,
    })
}

/// Ensure the backup directory exists, creating it if necessary.
fn ensure_backup_dir(backup_dir: &Path) -> AppResult<()> {
    std::fs::create_dir_all(backup_dir).map_err(|e| {
        AppError::Internal(format!(
            "Cannot create backup directory '{}': {e}",
            backup_dir.display()
        ))
    })
}

// ── Public service functions ──────────────────────────────────────────────────

/// Create an immediate backup of the live database using SQLite's `VACUUM INTO`
/// command, then rotate old backups so at most `retain_days` copies are kept.
///
/// `VACUUM INTO` produces a compact, defragmented copy that is safe to take
/// against a live database in WAL mode.
pub async fn create_backup(
    conn: &DatabaseConnection,
    backup_dir: &Path,
    retain_days: u32,
) -> AppResult<BackupEntry> {
    ensure_backup_dir(backup_dir)?;

    let filename = backup_filename();
    let backup_path = backup_dir.join(&filename);

    let path_str = backup_path
        .to_str()
        .ok_or_else(|| AppError::Internal("Backup path contains non-UTF-8 characters".into()))?;

    // Reject paths that would break the SQL literal.  In practice, backup_dir
    // comes from AppConfig (not user input) and backup_filename() is alphanumeric
    // only, so this guard is a belt-and-suspenders safety net.
    if path_str.contains('\'') {
        return Err(AppError::Internal(
            "Backup path must not contain single-quote characters".into(),
        ));
    }

    // VACUUM INTO does not support parameterised values; the path is embedded
    // as a SQL string literal.
    let sql = format!("VACUUM INTO '{path_str}'");
    conn.execute(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .map_err(|e| AppError::Internal(format!("VACUUM INTO failed: {e}")))?;

    tracing::info!(
        filename = %filename,
        path     = %backup_path.display(),
        "backup.created"
    );

    rotate_backups(backup_dir, retain_days)?;

    entry_from_path(&backup_path)
        .ok_or_else(|| AppError::Internal("Failed to read backup metadata after creation".into()))
}

/// Delete all but the `retain_days` most recent backup files.
///
/// Files are sorted lexicographically by filename (which equals chronological
/// order given the `YYYYMMDD_HHMMSS` timestamp prefix).
pub fn rotate_backups(backup_dir: &Path, retain_days: u32) -> AppResult<()> {
    let mut backups = list_backup_files(backup_dir)?;
    backups.sort_by(|a, b| a.filename.cmp(&b.filename));

    let to_delete = backups.len().saturating_sub(retain_days as usize);

    for entry in backups.iter().take(to_delete) {
        let path = backup_dir.join(&entry.filename);
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(
                path = %path.display(),
                error = %e,
                "backup.rotation_delete_failed"
            );
        } else {
            tracing::info!(filename = %entry.filename, "backup.rotated_out");
        }
    }

    Ok(())
}

/// List all backup files present in the backup directory, newest first.
pub fn list_backup_files(backup_dir: &Path) -> AppResult<Vec<BackupEntry>> {
    if !backup_dir.exists() {
        return Ok(vec![]);
    }

    let mut entries = std::fs::read_dir(backup_dir)
        .map_err(|e| {
            AppError::Internal(format!(
                "Cannot read backup directory '{}': {e}",
                backup_dir.display()
            ))
        })?
        .filter_map(|res| {
            let entry = res.ok()?;
            entry_from_path(&entry.path())
        })
        .collect::<Vec<_>>();

    // Newest first.
    entries.sort_by(|a, b| b.filename.cmp(&a.filename));
    Ok(entries)
}

/// Return a structured listing response for the admin endpoint.
pub fn list_backups(backup_dir: &Path, retain_days: u32) -> AppResult<BackupListResponse> {
    let backups = list_backup_files(backup_dir)?;
    Ok(BackupListResponse {
        backups,
        backup_dir: backup_dir.display().to_string(),
        retain_days,
    })
}

/// Stage a restore from `filename` inside `backup_dir`.
///
/// This writes a sentinel file (`{backup_dir}/.restore_pending`) whose content
/// is the absolute path of the chosen backup file.  On the **next server
/// startup**, `db::apply_pending_restore` reads this marker, copies the backup
/// over the live database file, and deletes the marker before opening any
/// connections.
///
/// The running process continues to serve requests normally after this call;
/// the restore takes effect only after a restart.
pub fn stage_restore(backup_dir: &Path, filename: &str) -> AppResult<PathBuf> {
    // Safety: reject any path traversal attempts.
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err(AppError::BadRequest(
            "filename must not contain path separators or '..'".into(),
        ));
    }
    if !filename.starts_with(BACKUP_PREFIX) || !filename.ends_with(BACKUP_EXT) {
        return Err(AppError::BadRequest(format!(
            "filename must match backup_YYYYMMDD_HHMMSS.{BACKUP_EXT}"
        )));
    }

    let backup_path = backup_dir.join(filename);
    if !backup_path.exists() {
        return Err(AppError::NotFound(format!("Backup '{filename}' not found")));
    }

    // Verify the file is a valid SQLite database (magic bytes check).
    validate_sqlite_file(&backup_path)?;

    // Write absolute path into the marker file.
    let abs_path = backup_path
        .canonicalize()
        .map_err(|e| AppError::Internal(format!("Cannot resolve backup path: {e}")))?;
    let marker = backup_dir.join(RESTORE_MARKER);
    std::fs::write(&marker, abs_path.to_str().unwrap_or(""))
        .map_err(|e| AppError::Internal(format!("Failed to write restore marker: {e}")))?;

    tracing::info!(
        filename = %filename,
        marker   = %marker.display(),
        "backup.restore_staged"
    );

    Ok(abs_path)
}

/// Check the SQLite file signature (first 16 bytes must be the SQLite magic string).
fn validate_sqlite_file(path: &Path) -> AppResult<()> {
    use std::io::Read;

    let mut f = std::fs::File::open(path)
        .map_err(|e| AppError::Internal(format!("Cannot open backup file: {e}")))?;

    let mut magic = [0u8; 16];
    f.read_exact(&mut magic)
        .map_err(|e| AppError::Internal(format!("Cannot read backup file header: {e}")))?;

    if &magic != b"SQLite format 3\0" {
        return Err(AppError::BadRequest(format!(
            "'{}' is not a valid SQLite database",
            path.file_name().unwrap_or_default().to_string_lossy()
        )));
    }
    Ok(())
}

// ── Startup restore logic ─────────────────────────────────────────────────────

/// Called at application startup (before any connections are opened) to check
/// whether a restore was staged by the previous run.
///
/// If the marker file exists the backup is copied over the target database
/// file.  On success the marker is deleted and a log line is emitted.
///
/// # Arguments
/// * `backup_dir` — the configured backup directory
/// * `db_file_path` — filesystem path of the primary SQLite database file
pub fn apply_pending_restore(backup_dir: &Path, db_file_path: &Path) -> Result<bool, String> {
    let marker = backup_dir.join(RESTORE_MARKER);
    if !marker.exists() {
        return Ok(false);
    }

    let content =
        std::fs::read_to_string(&marker).map_err(|e| format!("Cannot read restore marker: {e}"))?;
    let backup_path = PathBuf::from(content.trim());

    if !backup_path.exists() {
        std::fs::remove_file(&marker).ok();
        return Err(format!(
            "Staged restore file '{}' no longer exists — marker removed",
            backup_path.display()
        ));
    }

    // Validate before overwriting.
    validate_sqlite_file(&backup_path)
        .map_err(|e| format!("Staged restore file is corrupt: {e}"))?;

    // Ensure the target directory exists.
    if let Some(parent) = db_file_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("Cannot create db directory: {e}"))?;
    }

    std::fs::copy(&backup_path, db_file_path).map_err(|e| format!("Restore copy failed: {e}"))?;

    std::fs::remove_file(&marker)
        .map_err(|e| format!("Cannot remove restore marker after apply: {e}"))?;

    tracing::info!(
        backup = %backup_path.display(),
        target = %db_file_path.display(),
        "backup.restore_applied"
    );

    Ok(true)
}

/// Extract the filesystem path from a SQLite `DATABASE_URL`.
///
/// Handles the common formats used by sqlx/sea-orm:
/// - `sqlite:///absolute/path.db`  →  `/absolute/path.db`
/// - `sqlite://./relative.db`       →  `./relative.db`
/// - `sqlite://relative.db`         →  `relative.db`
pub fn db_path_from_url(url: &str) -> Option<PathBuf> {
    let stripped = url.strip_prefix("sqlite://")?;
    // Triple-slash absolute path: strip one leading slash.
    let path_str = if let Some(s) = stripped.strip_prefix('/') {
        format!("/{s}")
    } else {
        stripped.to_owned()
    };
    Some(PathBuf::from(path_str))
}
