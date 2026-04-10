pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Request types ─────────────────────────────────────────────────────────────

/// Submit a single result attempt for a participant within an event.
///
/// `attempt_no` is 1-based and must be unique per (event_id, participant_id).
/// If omitted the service auto-assigns the next attempt number.
#[derive(Deserialize, Clone)]
pub struct SubmitResultRequest {
    pub participant_id: i64,
    /// 1-based attempt number.  Omit to auto-assign.
    pub attempt_no: Option<i32>,
    /// Measured value in the given unit (e.g. 75432.0 milliseconds).
    pub value_numeric: f64,
    /// Unit of measurement: "milliseconds" | "feet" | "inches" |
    /// "seconds" | "meters" | "kilometers" | "kilograms" | "points"
    pub unit: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct ResultResponse {
    pub id: i64,
    pub event_id: i64,
    pub participant_id: i64,
    pub attempt_no: i32,
    pub value_numeric: f64,
    pub unit: String,
    pub reviewed_state: String,
    pub entered_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// One entry in the ranked standings for an event.
#[derive(Serialize, Clone)]
pub struct RankEntry {
    pub rank: u32,
    pub participant_id: i64,
    /// The best attempt value (lowest for time units, highest for distance/score).
    pub best_value: f64,
    pub unit: String,
    /// Attempt number of the best result.
    pub best_attempt_no: i32,
    /// RFC 3339 timestamp of the best result row (used as tie-breaker).
    pub best_recorded_at: String,
    /// Whether this participant advances under the given advancement rule.
    pub advances: bool,
}

#[derive(Serialize)]
pub struct RankingsResponse {
    pub event_id: i64,
    pub unit: String,
    pub advancement_rule: String,
    pub advancement_value: f64,
    pub total_participants: usize,
    pub rankings: Vec<RankEntry>,
}

// ── Review DTOs ───────────────────────────────────────────────────────────────

/// Submit a referee's decision on a result.
#[derive(Deserialize, Clone)]
pub struct SubmitReviewRequest {
    /// "approved" | "rejected"
    pub decision: String,
    pub comment: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ReviewResponse {
    pub id: i64,
    pub result_id: i64,
    pub referee_id: i64,
    pub decision: String,
    pub comment: Option<String>,
    pub reviewed_at: String,
    pub created_at: String,
}

// ── Arbitration DTOs ──────────────────────────────────────────────────────────

/// Event Director's binding arbitration decision on a conflicted result.
#[derive(Deserialize, Clone)]
pub struct ArbitrateRequest {
    /// "approved" | "rejected"
    pub decision: String,
    pub comment: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct ArbitrationResponse {
    pub id: i64,
    pub result_id: i64,
    pub arbitrated_by: i64,
    pub decision: String,
    pub comment: Option<String>,
    pub created_at: String,
}

// ── Correction DTOs ───────────────────────────────────────────────────────────

/// Request a value correction for a previously submitted result.
///
/// The original result row is **never modified** — the correction is an
/// immutable new row that, once approved, becomes the effective value.
#[derive(Deserialize, Clone)]
pub struct RequestCorrectionRequest {
    pub corrected_value: f64,
    /// Unit of the corrected value.
    pub corrected_unit: String,
    pub reason: Option<String>,
}

/// Resolve (approve or reject) a pending correction.
///
/// Only users with `events:write` (Event Director) may resolve corrections.
#[derive(Deserialize, Clone)]
pub struct ResolveCorrectionRequest {
    /// "approved" | "rejected"
    pub decision: String,
    pub comment: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct CorrectionResponse {
    pub id: i64,
    pub result_id: i64,
    pub corrected_value: f64,
    pub corrected_unit: String,
    pub requested_by: i64,
    pub reason: Option<String>,
    pub status: String,
    pub resolved_by: Option<i64>,
    pub resolved_at: Option<String>,
    pub created_at: String,
}

// ── Query filter ──────────────────────────────────────────────────────────────

#[derive(rocket::FromForm)]
pub struct RankingsQuery {
    /// Unit of measurement to rank — must match the submitted results.
    pub unit: String,
    /// Advancement rule: "top_n" | "percentile"
    pub advancement_rule: String,
    /// For top_n: number of participants who advance.
    /// For percentile: percentage threshold (0–100).
    pub advancement_value: f64,
}
