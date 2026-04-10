pub mod handlers;
pub mod service;

use rocket::FromForm;
use serde::{Deserialize, Serialize};

// ── Metric catalog ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct CreateMetricRequest {
    /// Unique, machine-readable name, e.g. "invoice_revenue_monthly".
    pub name: String,
    /// Human-readable definition: what the metric measures and how it is computed.
    pub definition: String,
    /// Display unit: "count" | "dollars" | "percentage" | "seconds" | etc.
    pub unit: Option<String>,
    /// "financial" | "operational" | "results" | "assets"
    pub category: String,
    /// User ID of the metric owner (accountable team member).
    pub owner_id: Option<i64>,
}

#[derive(Deserialize)]
pub struct UpdateMetricRequest {
    pub definition: String,
    pub unit: Option<String>,
    /// Mandatory reason for the version bump (stored in the audit trail).
    pub change_reason: String,
}

#[derive(Serialize, Clone)]
pub struct MetricResponse {
    pub id: i64,
    pub name: String,
    pub definition: String,
    pub unit: Option<String>,
    pub category: String,
    pub version: i32,
    pub owner_id: Option<i64>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone)]
pub struct MetricVersionResponse {
    pub id: i64,
    pub metric_id: i64,
    pub version: i32,
    pub definition: String,
    pub changed_by: Option<i64>,
    pub change_reason: Option<String>,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct MetricDetailResponse {
    pub metric: MetricResponse,
    pub history: Vec<MetricVersionResponse>,
}

// ── Analytics query parameters ─────────────────────────────────────────────────

/// Common time-window + drill-down parameters shared by all analytics endpoints.
#[derive(FromForm, Clone)]
pub struct AnalyticsQuery {
    /// Start of the time window (inclusive), ISO 8601 date "YYYY-MM-DD".
    pub start_date: Option<String>,
    /// End of the time window (inclusive), ISO 8601 date "YYYY-MM-DD".
    pub end_date: Option<String>,
    /// Time bucket granularity: "day" | "week" | "month" (default: "month").
    pub bucket: Option<String>,

    // Drill-down filters:
    /// Filter by a specific event ID.
    pub event_id: Option<i64>,
    /// Filter to championship-class events only ("true" / "false").
    pub is_championship_class: Option<bool>,
    /// Filter by event venue identifier.
    pub venue: Option<String>,
    /// Filter by asset category: "vehicle" | "equipment" | "facility" | "electronic" | "other".
    pub asset_category: Option<String>,
}

/// Trend-specific query — adds the metric name.
#[derive(FromForm)]
pub struct TrendQuery {
    /// Metric to trend:
    /// "invoice_revenue" | "invoice_count" | "payment_volume" | "payment_count" |
    /// "results_submitted" | "active_assets"
    pub metric: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub bucket: Option<String>,
    pub event_id: Option<i64>,
    pub is_championship_class: Option<bool>,
    pub venue: Option<String>,
    pub asset_category: Option<String>,
}

/// Funnel-specific query — adds funnel type.
#[derive(FromForm)]
pub struct FunnelQuery {
    /// "invoice_lifecycle" | "result_review" | "refund_approval"
    pub funnel_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub event_id: Option<i64>,
    pub is_championship_class: Option<bool>,
    pub venue: Option<String>,
}

/// Retention-specific query.
#[derive(FromForm)]
pub struct RetentionQuery {
    /// "event_participation" | "invoice_payment"
    pub retention_type: String,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    /// Number of follow-up periods to track (1–6, default 3).
    pub periods: Option<i32>,
}

/// Export query — wraps analytics type + all filters.
#[derive(FromForm)]
pub struct ExportQuery {
    /// "trends" | "funnel" | "retention"
    pub report_type: String,
    pub metric: Option<String>,
    pub funnel_type: Option<String>,
    pub retention_type: Option<String>,
    pub start_date: Option<String>,
    pub end_date: Option<String>,
    pub bucket: Option<String>,
    pub event_id: Option<i64>,
    pub is_championship_class: Option<bool>,
    pub venue: Option<String>,
    pub asset_category: Option<String>,
    pub periods: Option<i32>,
}

// ── Analytics response types ───────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct TrendPoint {
    pub bucket: String,
    /// Numeric value serialized as string for decimal precision.
    pub value: String,
}

#[derive(Serialize)]
pub struct TrendResponse {
    pub metric: String,
    pub bucket_size: String,
    pub start_date: String,
    pub end_date: String,
    pub data: Vec<TrendPoint>,
}

#[derive(Serialize, Clone)]
pub struct FunnelStep {
    pub stage: String,
    pub count: i64,
    /// Fraction of the first stage, e.g. "0.7500" (75 %).
    pub conversion_rate: String,
    /// Absolute drop-off from the previous stage.
    pub drop_off: i64,
}

#[derive(Serialize)]
pub struct FunnelResponse {
    pub funnel_type: String,
    pub start_date: String,
    pub end_date: String,
    pub steps: Vec<FunnelStep>,
}

#[derive(Serialize, Clone)]
pub struct RetentionRow {
    /// Cohort label, e.g. "2024-01".
    pub cohort: String,
    /// Number of entities in this cohort.
    pub cohort_size: i64,
    /// Retention counts for periods 1..N after the cohort period.
    pub periods: Vec<RetentionPeriod>,
}

#[derive(Serialize, Clone)]
pub struct RetentionPeriod {
    pub period: i32,
    pub count: i64,
    /// count / cohort_size as a decimal string.
    pub rate: String,
}

#[derive(Serialize)]
pub struct RetentionResponse {
    pub retention_type: String,
    pub start_date: String,
    pub end_date: String,
    pub rows: Vec<RetentionRow>,
}
