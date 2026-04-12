use rocket::{
    http::{ContentType, Header},
    serde::json::Json,
    State,
};
use sea_orm::DatabaseConnection;

use crate::errors::AppResult;
use crate::rbac::guards::{RequireAuditRead, RequireFinancialsRead, RequireFinancialsWrite};

use super::{
    service, CreateMetricRequest, ExportQuery, FunnelQuery, FunnelResponse, MetricDetailResponse,
    MetricResponse, RetentionQuery, RetentionResponse, TrendQuery, TrendResponse,
    UpdateMetricRequest,
};
use crate::middleware::rate_limit::RateLimitedToken;

// ── Metric catalog ─────────────────────────────────────────────────────────────

/// Create a new metric definition.
///
/// `name` must be unique across all active metrics.
/// The initial version (1) is seeded automatically.
///
/// **Required permission:** `financials:write`
#[post("/metrics", data = "<body>")]
pub async fn create_metric(
    guard: RequireFinancialsWrite,
    _rate_limit: RateLimitedToken,
    body: Json<CreateMetricRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<MetricResponse>> {
    let resp = service::create_metric(conn.inner(), guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

/// List all active metric definitions (most recently updated first).
///
/// **Required permission:** `financials:read`
#[get("/metrics")]
pub async fn list_metrics(
    _guard: RequireFinancialsRead,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<MetricResponse>>> {
    Ok(Json(service::list_metrics(conn.inner()).await?))
}

/// Get a single metric definition with its full version history.
///
/// **Required permission:** `financials:read`
#[get("/metrics/<id>")]
pub async fn get_metric(
    _guard: RequireFinancialsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<MetricDetailResponse>> {
    Ok(Json(service::get_metric(conn.inner(), id).await?))
}

/// Update a metric's definition and/or unit, creating a new version entry.
///
/// `change_reason` is mandatory and stored in the audit trail.
///
/// **Required permission:** `financials:write`
#[put("/metrics/<id>", data = "<body>")]
pub async fn update_metric(
    guard: RequireFinancialsWrite,
    _rate_limit: RateLimitedToken,
    id: i64,
    body: Json<UpdateMetricRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<MetricResponse>> {
    let resp = service::update_metric(conn.inner(), id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

// ── Analytics queries ──────────────────────────────────────────────────────────

/// Compute a time-series trend for a named metric.
///
/// `metric` must be one of:
/// `invoice_revenue` | `invoice_count` | `payment_volume` | `payment_count` |
/// `results_submitted` | `active_assets`
///
/// Optional filters: `start_date`, `end_date` (ISO 8601), `bucket` (`day` | `week` | `month`),
/// `event_id`, `is_championship_class`, `venue`, `asset_category`.
///
/// **Required permission:** `audit:read`
#[get("/analytics/trends?<q..>")]
pub async fn trends(
    _guard: RequireAuditRead,
    q: TrendQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<TrendResponse>> {
    Ok(Json(service::compute_trends(conn.inner(), q).await?))
}

/// Compute a conversion funnel.
///
/// `funnel_type` must be one of:
/// `invoice_lifecycle` | `result_review` | `refund_approval`
///
/// Optional filters: `start_date`, `end_date`, `event_id`, `is_championship_class`, `venue`.
///
/// **Required permission:** `audit:read`
#[get("/analytics/funnel?<q..>")]
pub async fn funnel(
    _guard: RequireAuditRead,
    q: FunnelQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<FunnelResponse>> {
    Ok(Json(service::compute_funnel(conn.inner(), q).await?))
}

/// Compute a retention matrix.
///
/// `retention_type` must be one of:
/// `event_participation` | `invoice_payment`
///
/// Optional filters: `start_date`, `end_date`, `periods` (1–6, default 3).
///
/// **Required permission:** `audit:read`
#[get("/analytics/retention?<q..>")]
pub async fn retention(
    _guard: RequireAuditRead,
    q: RetentionQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<RetentionResponse>> {
    Ok(Json(service::compute_retention(conn.inner(), q).await?))
}

// ── CSV download helper ────────────────────────────────────────────────────────

pub struct CsvDownload {
    filename: &'static str,
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

// ── CSV export ─────────────────────────────────────────────────────────────────

/// Export analytics data as a CSV file.
///
/// `report_type` must be one of: `trends` | `funnel` | `retention`
///
/// Depending on `report_type`, supply the corresponding parameters:
/// - `trends`: requires `metric`
/// - `funnel`: requires `funnel_type`
/// - `retention`: requires `retention_type`
///
/// All time-window and drill-down parameters are supported.
///
/// **Required permission:** `audit:read`
#[get("/analytics/export?<q..>")]
pub async fn export(
    _guard: RequireAuditRead,
    q: ExportQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<CsvDownload> {
    let csv = service::export_csv(conn.inner(), q).await?;
    Ok(CsvDownload {
        filename: "export.csv",
        body: csv,
    })
}
