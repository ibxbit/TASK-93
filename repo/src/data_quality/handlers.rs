use crate::middleware::rate_limit::RateLimitedToken;
use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::errors::AppResult;
use crate::rbac::guards::RequireAuditRead;

use super::{service, ScanListQuery, ScanReport, ScanRequest, ScanSummary};

/// Run a data-quality scan on the specified entity and return the full anomaly report.
///
/// ## Supported entities
/// `events` | `assets` | `vehicles` | `invoices` | `results` | `payments`
///
/// ## Supported checks
/// - `missing_fields` — flags records where required fields are null or empty.
/// - `outliers` — flags records whose numeric field values deviate more than
///   `zscore_threshold` standard deviations from the column mean (default threshold: `3.0`).
/// - `duplicates` — flags records that share identical values across the hash fields,
///   detected via FNV-1a stable hashing.
///
/// ## Configurable thresholds
/// - `zscore_threshold` — outlier sensitivity (default `3.0`, must be > 0).
/// - `numeric_fields` — override which columns are analysed for outliers.
/// - `hash_fields` — override which columns are combined for duplicate detection.
///
/// Scan results are persisted and retrievable via `GET /data-quality/scans/<id>`.
///
/// **Required permission:** `audit:read`
#[post("/data-quality/scans", data = "<body>")]
pub async fn run_scan(
    guard: RequireAuditRead,
    _rate_limit: RateLimitedToken,
    body: Json<ScanRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<ScanReport>> {
    Ok(Json(
        service::run_scan(conn.inner(), guard.0.user_id, body.into_inner()).await?,
    ))
}

/// List all previously run scans, newest first.
///
/// Optional query parameters:
/// - `entity=invoices` — filter by entity type.
/// - `limit=20` — maximum results (default: 20, max: 100).
///
/// Returns lightweight summaries without anomaly details.
/// Use `GET /data-quality/scans/<id>` to retrieve the full report.
///
/// **Required permission:** `audit:read`
#[get("/data-quality/scans?<q..>")]
pub async fn list_scans(
    _guard: RequireAuditRead,
    q: ScanListQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<ScanSummary>>> {
    Ok(Json(service::list_scans(conn.inner(), q).await?))
}

/// Retrieve the full anomaly report for a previously run scan.
///
/// **Required permission:** `audit:read`
#[get("/data-quality/scans/<id>")]
pub async fn get_scan(
    _guard: RequireAuditRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<ScanReport>> {
    Ok(Json(service::get_scan(conn.inner(), id).await?))
}
