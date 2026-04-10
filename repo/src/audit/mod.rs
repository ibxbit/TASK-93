pub mod handlers;
pub mod service;

use rocket::FromForm;
use serde::{Deserialize, Serialize};

/// A single entry from the unified audit trail.
#[derive(Debug, Serialize)]
pub struct AuditLogEntry {
    pub id: i64,
    /// User who triggered the operation.
    pub actor_id: i64,
    /// Dot-namespaced action, e.g. `"invoice.issued"`.
    pub action: String,
    /// Domain object type, e.g. `"invoice"`, `"vehicle"`.
    pub entity_type: String,
    /// Primary key of the affected entity.
    pub entity_id: i64,
    /// Entity state snapshot after the operation (parsed JSON).
    pub snapshot: serde_json::Value,
    /// Supplementary context (parsed JSON).
    pub metadata: serde_json::Value,
    /// RFC 3339 timestamp.
    pub created_at: String,
}

/// Query parameters accepted by `GET /audit/logs`.
#[derive(Debug, Default, Deserialize, FromForm)]
pub struct AuditLogQuery {
    /// Filter by the acting user.
    pub actor_id: Option<i64>,
    /// Filter by entity type (e.g. `"invoice"`, `"vehicle"`).
    pub entity_type: Option<String>,
    /// Filter by specific entity primary key (requires `entity_type`).
    pub entity_id: Option<i64>,
    /// Filter by action prefix or exact match (e.g. `"invoice.issued"`).
    pub action: Option<String>,
    /// Inclusive lower bound — RFC 3339 timestamp.
    pub from: Option<String>,
    /// Inclusive upper bound — RFC 3339 timestamp.
    pub to: Option<String>,
    /// Maximum rows to return (default 100, max 500).
    pub limit: Option<u64>,
    /// Rows to skip for pagination.
    pub offset: Option<u64>,
}
