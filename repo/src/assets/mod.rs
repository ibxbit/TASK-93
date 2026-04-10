pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Create / update ───────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct CreateAssetRequest {
    pub asset_code: String,
    /// "vehicle" | "equipment" | "facility" | "electronic" | "other"
    pub category: String,
    pub brand: String,
    pub model: String,
    pub serial_number: Option<String>,
    /// "in_service" | "out_for_repair" | "retired"  (default: in_service)
    pub status: Option<String>,
    pub owner_id: Option<i64>,
    pub responsible_person_id: Option<i64>,
    /// Original acquisition cost.
    pub procurement_cost: Option<f64>,
    /// ISO 8601 date string "YYYY-MM-DD".
    pub procurement_date: Option<String>,
    /// Straight-line depreciation period in months (to full write-off).
    pub useful_life_months: Option<i32>,
    pub notes: Option<String>,
}

/// Patch-style update — only supplied fields are written.
#[derive(Deserialize, Clone, Default)]
pub struct UpdateAssetRequest {
    pub category: Option<String>,
    pub brand: Option<String>,
    pub model: Option<String>,
    pub serial_number: Option<String>,
    pub owner_id: Option<i64>,
    pub responsible_person_id: Option<i64>,
    pub procurement_cost: Option<f64>,
    pub procurement_date: Option<String>,
    pub useful_life_months: Option<i32>,
    pub notes: Option<String>,
}

#[derive(Deserialize, Clone)]
pub struct StatusUpdateRequest {
    /// "in_service" | "out_for_repair" | "retired"
    pub status: String,
}

// ── Bulk import / export ──────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct BulkImportRequest {
    pub assets: Vec<CreateAssetRequest>,
}

#[derive(Serialize)]
pub struct BulkImportResponse {
    pub imported: usize,
    pub skipped: usize,
    pub errors: Vec<ImportRowError>,
}

#[derive(Serialize)]
pub struct ImportRowError {
    /// 0-based index in the submitted array.
    pub index: usize,
    pub asset_code: Option<String>,
    pub reason: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct AssetResponse {
    pub id: i64,
    pub asset_code: String,
    pub category: String,
    pub brand: String,
    pub model: String,
    pub serial_number: Option<String>,
    pub status: String,
    pub owner_id: Option<i64>,
    pub responsible_person_id: Option<i64>,
    pub procurement_cost: Option<f64>,
    pub procurement_date: Option<String>,
    pub useful_life_months: Option<i32>,
    // Straight-line depreciation — computed, not stored.
    pub monthly_depreciation: Option<f64>,
    pub current_book_value: Option<f64>,
    pub fully_depreciated: bool,
    pub notes: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize)]
pub struct AuditEntry {
    pub id: i64,
    pub asset_id: i64,
    pub changed_by: i64,
    pub change_type: String,
    /// JSON snapshot of the full asset state after this change.
    pub snapshot: serde_json::Value,
    pub changed_at: String,
}

// ── Query filter ──────────────────────────────────────────────────────────────

#[derive(rocket::FromForm)]
pub struct AssetFilterQuery {
    pub category: Option<String>,
    pub status: Option<String>,
}
