use rocket::{
    http::{ContentType, Header},
    serde::json::Json,
    State,
};
use sea_orm::DatabaseConnection;

// ── CSV download helper ────────────────────────────────────────────────────────

pub struct CsvDownload {
    filename: String,
    body: String,
}

impl<'r> rocket::response::Responder<'r, 'static> for CsvDownload {
    fn respond_to(self, _: &'r rocket::Request<'_>) -> rocket::response::Result<'static> {
        rocket::Response::build()
            .header(ContentType::new("text", "csv"))
            .header(Header::new(
                "Content-Disposition",
                format!("attachment; filename=\"{}\"", self.filename),
            ))
            .sized_body(self.body.len(), std::io::Cursor::new(self.body))
            .ok()
    }
}

use crate::errors::AppResult;
use crate::rbac::guards::{
    RequireAuditRead, RequireEventsRead, RequireEventsWrite, RequireParticipantsWrite,
    RequireRefereesWrite,
};

use super::{
    service, ArbitrateRequest, ArbitrationResponse, CorrectionResponse, RankingsQuery,
    RankingsResponse, RequestCorrectionRequest, ResolveCorrectionRequest, ResultResponse,
    ReviewResponse, SubmitResultRequest, SubmitReviewRequest,
};

// ══════════════════════════════════════════════════════════════════════════════
// Result submission
// ══════════════════════════════════════════════════════════════════════════════

/// Submit a result attempt for a participant in the given event.
///
/// `attempt_no` is 1-based and unique per (event_id, participant_id).
/// Omit to auto-assign as the next sequential attempt.
///
/// **Required permission:** `participants:write`
#[post("/events/<event_id>/results", data = "<body>")]
pub async fn submit_result(
    guard: RequireParticipantsWrite,
    event_id: i64,
    body: Json<SubmitResultRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<ResultResponse>> {
    let resp =
        service::submit_result(conn.inner(), event_id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Rankings
// ══════════════════════════════════════════════════════════════════════════════

/// Return deterministic rankings for an event.
///
/// Rankings reflect the effective value for each participant — the latest
/// approved correction supersedes the original submitted value.
/// Only non-rejected results are included.
///
/// **Query params (all required):**
/// - `unit` — unit to rank against (must match submitted/corrected results)
/// - `advancement_rule` — `top_n` | `percentile`
/// - `advancement_value` — N for top_n; 0–100 for percentile
///
/// **Required permission:** `events:read`
#[get("/events/<event_id>/rankings?<query..>")]
pub async fn get_rankings(
    _guard: RequireEventsRead,
    event_id: i64,
    query: RankingsQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<RankingsResponse>> {
    let resp = service::get_rankings(conn.inner(), event_id, query).await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Reviews
// ══════════════════════════════════════════════════════════════════════════════

/// Submit a referee's decision on a result.
///
/// Championship class events require ≥ 2 referee reviews before auto-approval.
/// If reviews conflict (any rejection while others approve), the result stays
/// `pending` until an Event Director issues arbitration.
///
/// **Required permission:** `referees:write`
#[post("/events/<event_id>/results/<result_id>/reviews", data = "<body>")]
pub async fn submit_review(
    guard: RequireRefereesWrite,
    event_id: i64,
    result_id: i64,
    body: Json<SubmitReviewRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<ReviewResponse>> {
    let resp = service::submit_review(
        conn.inner(),
        event_id,
        result_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// List all referee reviews for a result, in submission order.
///
/// **Required permission:** `events:read`
#[get("/events/<event_id>/results/<result_id>/reviews")]
pub async fn list_reviews(
    _guard: RequireEventsRead,
    event_id: i64,
    result_id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<ReviewResponse>>> {
    let resp = service::list_reviews(conn.inner(), event_id, result_id).await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Arbitration
// ══════════════════════════════════════════════════════════════════════════════

/// Issue a binding arbitration decision on a pending or rejected result.
///
/// The Event Director's decision overrides all prior referee reviews and is
/// final.  Once arbitrated, a result cannot be re-arbitrated.
///
/// **Required permission:** `events:write` (Event Director or Administrator)
#[post("/events/<event_id>/results/<result_id>/arbitrate", data = "<body>")]
pub async fn arbitrate_result(
    guard: RequireEventsWrite,
    event_id: i64,
    result_id: i64,
    body: Json<ArbitrateRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<ArbitrationResponse>> {
    let resp = service::arbitrate_result(
        conn.inner(),
        event_id,
        result_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Corrections
// ══════════════════════════════════════════════════════════════════════════════

/// Request a value correction for a submitted result.
///
/// The original result row is **never modified**.  The correction is an
/// immutable new record that, once approved, becomes the effective value
/// used in rankings.  Only one pending correction per result is allowed.
///
/// **Required permission:** `participants:write`
#[post("/events/<event_id>/results/<result_id>/corrections", data = "<body>")]
pub async fn request_correction(
    guard: RequireParticipantsWrite,
    event_id: i64,
    result_id: i64,
    body: Json<RequestCorrectionRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<CorrectionResponse>> {
    let resp = service::request_correction(
        conn.inner(),
        event_id,
        result_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// List the full correction history for a result, oldest first.
///
/// **Required permission:** `events:read`
#[get("/events/<event_id>/results/<result_id>/corrections")]
pub async fn list_corrections(
    _guard: RequireEventsRead,
    event_id: i64,
    result_id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<CorrectionResponse>>> {
    let resp = service::list_corrections(conn.inner(), event_id, result_id).await?;
    Ok(Json(resp))
}

/// Approve or reject a pending correction.
///
/// Only one resolution is permitted — the correction row transitions from
/// `pending` to `approved` or `rejected` exactly once.  Approved corrections
/// immediately affect rankings.
///
/// **Required permission:** `events:write` (Event Director or Administrator)
#[post(
    "/events/<event_id>/results/<result_id>/corrections/<correction_id>/resolve",
    data = "<body>"
)]
pub async fn resolve_correction(
    guard: RequireEventsWrite,
    event_id: i64,
    result_id: i64,
    correction_id: i64,
    body: Json<ResolveCorrectionRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<CorrectionResponse>> {
    let resp = service::resolve_correction(
        conn.inner(),
        event_id,
        result_id,
        correction_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Export results as CSV
// ══════════════════════════════════════════════════════════════════════════════

/// Export all results for an event as a UTF-8 CSV file download.
///
/// Returns every result row with its submitted value, effective value (applying
/// the latest approved correction if any), and current review state.
///
/// **Required permission:** `audit:read`
#[get("/events/<event_id>/results/export")]
pub async fn export_results(
    _guard: RequireAuditRead,
    event_id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<CsvDownload> {
    let csv = service::export_results_csv(conn.inner(), event_id).await?;
    Ok(CsvDownload {
        filename: format!("results_event_{event_id}.csv"),
        body: csv,
    })
}
