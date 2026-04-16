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

/// Generate a timestamped backup filename, e.g. `backup_20260407_230000_123456.sqlite`.
/// Microsecond precision prevents filename collisions when multiple backups are
/// triggered within the same second (e.g. concurrent test runs).
fn backup_filename() -> String {
    let now = Utc::now();
    let ts = now.format("%Y%m%d_%H%M%S");
    let us = now.format("%6f");
    format!("{BACKUP_PREFIX}{ts}_{us}.{BACKUP_EXT}")
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

// ══════════════════════════════════════════════════════════════════════════════
// Native Rust unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    // ── db_path_from_url ────────────────────────────────────────────────────

    #[test]
    fn db_path_from_url_absolute() {
        let p = db_path_from_url("sqlite:///app/data/motorsport.db").unwrap();
        assert_eq!(p, PathBuf::from("/app/data/motorsport.db"));
    }

    #[test]
    fn db_path_from_url_relative_dot() {
        let p = db_path_from_url("sqlite://./local.db").unwrap();
        assert_eq!(p, PathBuf::from("./local.db"));
    }

    #[test]
    fn db_path_from_url_relative_bare() {
        let p = db_path_from_url("sqlite://data.db").unwrap();
        assert_eq!(p, PathBuf::from("data.db"));
    }

    #[test]
    fn db_path_from_url_rejects_non_sqlite() {
        assert!(db_path_from_url("postgres://localhost/db").is_none());
        assert!(db_path_from_url("mysql://host/db").is_none());
    }

    #[test]
    fn db_path_from_url_rejects_empty() {
        assert!(db_path_from_url("").is_none());
    }

    // ── backup_filename format ──────────────────────────────────────────────

    #[test]
    fn backup_filename_starts_with_prefix() {
        let name = backup_filename();
        assert!(name.starts_with(BACKUP_PREFIX));
    }

    #[test]
    fn backup_filename_ends_with_extension() {
        let name = backup_filename();
        assert!(name.ends_with(&format!(".{BACKUP_EXT}")));
    }

    #[test]
    fn backup_filename_has_reasonable_length() {
        let name = backup_filename();
        // backup_YYYYMMDD_HHMMSS_microseconds.sqlite → well over 20 chars
        assert!(name.len() > 20, "backup filename too short: {name}");
    }

    // ── stage_restore input validation ──────────────────────────────────────

    #[test]
    fn stage_restore_rejects_path_traversal() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "../evil.sqlite").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn stage_restore_rejects_forward_slash() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "sub/backup.sqlite").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn stage_restore_rejects_backslash() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "sub\\backup.sqlite").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn stage_restore_rejects_wrong_prefix() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "evil_20260101_000000.sqlite").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn stage_restore_rejects_wrong_extension() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "backup_20260101_000000.db").unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn stage_restore_rejects_nonexistent_file() {
        let tmp = tempdir();
        let err = stage_restore(&tmp, "backup_20260101_000000.sqlite").unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    // ── validate_sqlite_file ────────────────────────────────────────────────

    #[test]
    fn validate_sqlite_rejects_non_sqlite_file() {
        let tmp = tempdir();
        let path = tmp.join("fake.sqlite");
        fs::write(&path, b"This is not a real SQLite file at all!").unwrap();
        let err = validate_sqlite_file(&path).unwrap_err();
        assert!(matches!(err, AppError::BadRequest(_)));
    }

    #[test]
    fn validate_sqlite_rejects_empty_file() {
        let tmp = tempdir();
        let path = tmp.join("empty.sqlite");
        fs::write(&path, b"").unwrap();
        assert!(validate_sqlite_file(&path).is_err());
    }

    #[test]
    fn validate_sqlite_accepts_valid_header() {
        let tmp = tempdir();
        let path = tmp.join("valid.sqlite");
        // Write the 16-byte SQLite magic header followed by garbage.
        let mut data = b"SQLite format 3\0".to_vec();
        data.extend_from_slice(&[0u8; 100]);
        fs::write(&path, &data).unwrap();
        assert!(validate_sqlite_file(&path).is_ok());
    }

    // ── rotate_backups ──────────────────────────────────────────────────────

    #[test]
    fn rotate_backups_keeps_at_most_n_files() {
        let tmp = tempdir();
        // Create 5 fake backup files with valid naming.
        for i in 0..5 {
            let name = format!("backup_20260101_00000{i}.sqlite");
            fs::write(tmp.join(&name), format!("backup-data-{i}")).unwrap();
        }
        rotate_backups(&tmp, 3).unwrap();
        let remaining = list_backup_files(&tmp).unwrap();
        assert!(remaining.len() <= 3, "expected <=3 files, got {}", remaining.len());
    }

    #[test]
    fn rotate_backups_on_empty_dir_is_ok() {
        let tmp = tempdir();
        assert!(rotate_backups(&tmp, 7).is_ok());
    }

    // ── Helper ──────────────────────────────────────────────────────────────

    fn tempdir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("backup_test_{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
