use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::errors::AppResult;
use crate::rbac::guards::RequireAuditRead;

use super::{service, AuditLogEntry, AuditLogQuery};

/// List audit log entries with optional filters.
///
/// Results are returned newest-first.  Available query parameters:
/// - `actor_id` — filter by acting user ID
/// - `entity_type` — e.g. `invoice`, `vehicle`, `payment`, `event`, `result`
/// - `entity_id` — filter by specific entity (requires `entity_type`)
/// - `action` — exact action name, e.g. `invoice.issued`
/// - `from` — inclusive lower bound (RFC 3339)
/// - `to` — inclusive upper bound (RFC 3339)
/// - `limit` — max rows (default 100, capped at 500)
/// - `offset` — pagination offset
///
/// **Required permission:** `audit:read`
#[get("/audit/logs?<q..>")]
pub async fn list_audit_logs(
    _guard: RequireAuditRead,
    q: AuditLogQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<AuditLogEntry>>> {
    Ok(Json(service::list(conn.inner(), q).await?))
}

/// Retrieve a single audit log entry by ID.
///
/// **Required permission:** `audit:read`
#[get("/audit/logs/<id>")]
pub async fn get_audit_log(
    _guard: RequireAuditRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<AuditLogEntry>> {
    Ok(Json(service::get(conn.inner(), id).await?))
}
