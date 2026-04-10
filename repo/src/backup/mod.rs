pub mod handlers;
pub mod scheduler;
pub mod service;

use serde::Serialize;

/// Metadata for a single backup file.
#[derive(Debug, Serialize, Clone)]
pub struct BackupEntry {
    /// Filename only (no path), e.g. `backup_20260407_230000.sqlite`.
    pub filename: String,
    /// Uncompressed file size in bytes.
    pub size_bytes: u64,
    /// ISO 8601 timestamp extracted from the filename.
    pub created_at: String,
}

/// Response returned by the list-backups endpoint.
#[derive(Debug, Serialize)]
pub struct BackupListResponse {
    pub backups: Vec<BackupEntry>,
    pub backup_dir: String,
    pub retain_days: u32,
}

/// Response returned by the create-backup and restore endpoints.
#[derive(Debug, Serialize)]
pub struct BackupActionResponse {
    pub status: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupEntry>,
}
