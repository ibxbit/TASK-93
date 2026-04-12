use crate::middleware::rate_limit::RateLimitedToken;
use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::crypto::Cipher;
use crate::errors::AppResult;
use crate::rbac::guards::{RequireEventsRead, RequireEventsWrite};

use super::{
    service, CreateVehicleRequest, StatusTransitionRequest, UpdateVehicleRequest,
    VehicleAuditEntry, VehicleFilterQuery, VehicleResponse,
};

/// Create a vehicle in the lifecycle registry.
///
/// `vin` and `registration_id` are AES-256-GCM encrypted at rest.
/// VIN is normalised to uppercase and must be exactly 17 valid ISO 3779
/// characters (I, O, Q excluded).
///
/// **Required permission:** `events:write`
#[post("/vehicles", data = "<body>")]
pub async fn create_vehicle(
    guard: RequireEventsWrite,
    _rate_limit: RateLimitedToken,
    body: Json<CreateVehicleRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<VehicleResponse>> {
    let resp = service::create_vehicle(
        conn.inner(),
        cipher.inner(),
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Get a single vehicle by ID.
///
/// `vin` and `registration_id` are decrypted before being returned.
///
/// **Required permission:** `events:read`
#[get("/vehicles/<id>")]
pub async fn get_vehicle(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<VehicleResponse>> {
    Ok(Json(
        service::get_vehicle(conn.inner(), cipher.inner(), id).await?,
    ))
}

/// List vehicles with optional `status` and `make` filters.
///
/// `vin` and `registration_id` are decrypted in each result.
///
/// **Required permission:** `events:read`
#[get("/vehicles?<filter..>")]
pub async fn list_vehicles(
    _guard: RequireEventsRead,
    filter: VehicleFilterQuery,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<Vec<VehicleResponse>>> {
    Ok(Json(
        service::list_vehicles(conn.inner(), cipher.inner(), filter).await?,
    ))
}

/// Patch-update mutable vehicle fields.
///
/// **VIN is immutable** after creation.  Updating `registration_id` re-encrypts
/// it transparently.  **Mileage** may only increase.  **Sold** vehicles are
/// fully immutable.
///
/// **Required permission:** `events:write`
#[put("/vehicles/<id>", data = "<body>")]
pub async fn update_vehicle(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<UpdateVehicleRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<VehicleResponse>> {
    let resp = service::update_vehicle(
        conn.inner(),
        cipher.inner(),
        id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Perform a lifecycle status transition.
///
/// **Required permission:** `events:write`
#[post("/vehicles/<id>/status", data = "<body>")]
pub async fn transition_status(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<StatusTransitionRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<VehicleResponse>> {
    let resp = service::transition_status(
        conn.inner(),
        cipher.inner(),
        id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Return the full audit history for a vehicle, oldest entry first.
///
/// Sensitive identifiers (`vin`, `registration_id`) appear as `[REDACTED]` in
/// every snapshot — they are masked at write time and never stored in plaintext
/// in the audit log.
///
/// **Required permission:** `events:read`
#[get("/vehicles/<id>/history")]
pub async fn get_history(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<VehicleAuditEntry>>> {
    Ok(Json(service::get_history(conn.inner(), id).await?))
}
