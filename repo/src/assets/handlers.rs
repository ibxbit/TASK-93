use crate::middleware::rate_limit::RateLimitedToken;
use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::crypto::Cipher;
use crate::errors::AppResult;
use crate::rbac::guards::{RequireEventsRead, RequireEventsWrite};

use super::{
    service, AssetFilterQuery, AssetResponse, AuditEntry, BulkImportRequest, BulkImportResponse,
    CreateAssetRequest, StatusUpdateRequest, UpdateAssetRequest,
};

// ── CRUD ──────────────────────────────────────────────────────────────────────

/// Create a new asset in the asset register.
///
/// `serial_number` is AES-256-GCM encrypted at rest; the plaintext value is
/// returned in the response but never stored in cleartext.
///
/// **Required permission:** `events:write`
#[post("/assets", data = "<body>")]
pub async fn create_asset(
    guard: RequireEventsWrite,
    _rate_limit: RateLimitedToken,
    body: Json<CreateAssetRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<AssetResponse>> {
    let resp = service::create_asset(
        conn.inner(),
        cipher.inner(),
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Get a single asset by ID, including computed depreciation values.
///
/// `serial_number` is decrypted before being returned.
///
/// **Required permission:** `events:read`
#[get("/assets/<id>")]
pub async fn get_asset(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<AssetResponse>> {
    Ok(Json(
        service::get_asset(conn.inner(), cipher.inner(), id).await?,
    ))
}

/// List assets with optional `category` and/or `status` query filters.
///
/// `serial_number` is decrypted in each result.
///
/// **Required permission:** `events:read`
#[get("/assets?<filter..>")]
pub async fn list_assets(
    _guard: RequireEventsRead,
    filter: AssetFilterQuery,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<Vec<AssetResponse>>> {
    Ok(Json(
        service::list_assets(conn.inner(), cipher.inner(), filter).await?,
    ))
}

/// Patch-update an asset.  Only supplied fields are written.
///
/// Send `""` for any nullable text field to clear it.
/// Updating `serial_number` re-encrypts it transparently.
///
/// **Required permission:** `events:write`
#[put("/assets/<id>", data = "<body>")]
pub async fn update_asset(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<UpdateAssetRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<AssetResponse>> {
    let resp = service::update_asset(
        conn.inner(),
        cipher.inner(),
        id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Update only the operational status of an asset.
///
/// Retired assets cannot be brought back into service (returns 409).
///
/// **Required permission:** `events:write`
#[patch("/assets/<id>/status", data = "<body>")]
pub async fn update_status(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<StatusUpdateRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<AssetResponse>> {
    let resp = service::update_status(
        conn.inner(),
        cipher.inner(),
        id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

// ── Version history ───────────────────────────────────────────────────────────

/// Return the full audit history for an asset, oldest change first.
///
/// Sensitive identifiers (`serial_number`) appear as `[REDACTED]` in every
/// snapshot — they are masked at write time and never stored in plaintext in
/// the audit log.
///
/// **Required permission:** `events:read`
#[get("/assets/<id>/history")]
pub async fn get_history(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<AuditEntry>>> {
    Ok(Json(service::get_history(conn.inner(), id).await?))
}

// ── Bulk operations ───────────────────────────────────────────────────────────

/// Export all assets as a flat JSON array.
///
/// Includes computed depreciation values for each asset.
/// `serial_number` values are decrypted before being returned.
///
/// **Required permission:** `events:read`
#[get("/assets/export")]
pub async fn export_assets(
    _guard: RequireEventsRead,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<Vec<AssetResponse>>> {
    Ok(Json(
        service::export_assets(conn.inner(), cipher.inner()).await?,
    ))
}

/// Bulk-import assets from a JSON array.
///
/// Each record is processed independently:
/// - Required fields (`asset_code`, `category`, `brand`, `model`) are validated.
/// - Deduplication key: `(asset_code, serial_number)`.
///   - Exact match on both → **skipped** (silent deduplicate).
///   - `asset_code` matches but `serial_number` differs → **error** (collision).
/// - Rows that fail validation or collide are reported in `errors`; all
///   other rows are committed regardless.
/// - `serial_number` is AES-256-GCM encrypted at rest.
///
/// **Required permission:** `events:write`
#[post("/assets/import", data = "<body>")]
pub async fn import_assets(
    guard: RequireEventsWrite,
    body: Json<BulkImportRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<BulkImportResponse>> {
    let resp = service::import_assets(
        conn.inner(),
        cipher.inner(),
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}
