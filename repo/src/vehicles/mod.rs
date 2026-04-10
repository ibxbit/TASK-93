pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Create ────────────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct CreateVehicleRequest {
    /// Optional link to an existing asset in the asset register.
    pub asset_id: Option<i64>,
    /// ISO 3779 VIN — exactly 17 alphanumeric characters (I, O, Q excluded).
    pub vin: String,
    pub registration_id: String,
    pub make: String,
    pub model: String,
    /// Four-digit model year.
    pub year: i32,
    pub color: Option<String>,
    /// Initial odometer reading (default 0).
    #[serde(default)]
    pub mileage: i64,
    /// Number of prior ownership transfers (default 0).
    #[serde(default)]
    pub title_transfer_count: i32,
    pub notes: Option<String>,
}

// ── Update ────────────────────────────────────────────────────────────────────

/// Patch-style update.  VIN and status are immutable via this endpoint.
///
/// `mileage` may only increase; supplying a value lower than the current
/// odometer returns 422.
#[derive(Deserialize, Clone, Default)]
pub struct UpdateVehicleRequest {
    pub asset_id: Option<i64>,
    pub registration_id: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub year: Option<i32>,
    pub color: Option<String>,
    /// New odometer reading — must be ≥ current value.
    pub mileage: Option<i64>,
    pub notes: Option<String>,
}

// ── Status transition ─────────────────────────────────────────────────────────

/// Request a lifecycle status transition.
///
/// `reason` is **mandatory** when transitioning to `delisted` or `sold`.
/// For other transitions it is recorded in the audit log if supplied.
#[derive(Deserialize, Clone)]
pub struct StatusTransitionRequest {
    /// Target lifecycle state: "published" | "delisted" | "sold"
    pub status: String,
    pub reason: Option<String>,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct VehicleResponse {
    pub id: i64,
    pub asset_id: Option<i64>,
    pub vin: String,
    pub registration_id: String,
    pub make: String,
    pub model: String,
    pub year: i32,
    pub color: Option<String>,
    pub mileage: i64,
    pub title_transfer_count: i32,
    pub status: String,
    /// Populated for `delisted` and `sold` vehicles.
    pub status_reason: Option<String>,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct VehicleAuditEntry {
    pub id: i64,
    pub vehicle_id: i64,
    pub changed_by: i64,
    pub change_type: String,
    /// Full JSON snapshot of the vehicle state after this change.
    pub snapshot: serde_json::Value,
    pub changed_at: String,
}

// ── Query filter ──────────────────────────────────────────────────────────────

#[derive(rocket::FromForm)]
pub struct VehicleFilterQuery {
    /// "draft" | "published" | "delisted" | "sold"
    pub status: Option<String>,
    pub make: Option<String>,
}
