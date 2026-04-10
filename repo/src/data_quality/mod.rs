pub mod handlers;
pub mod service;

use rocket::FromForm;
use serde::{Deserialize, Serialize};

/// Request body for a data-quality scan.
#[derive(Deserialize, Clone)]
pub struct ScanRequest {
    /// Entity to scan.
    /// Supported: `"events"` | `"assets"` | `"vehicles"` | `"invoices"` | `"results"` | `"payments"`
    pub entity: String,
    /// Checks to run — at least one required.
    /// Supported: `"missing_fields"` | `"outliers"` | `"duplicates"`
    pub checks: Vec<String>,
    /// Z-score threshold for outlier detection (default: `3.0`, must be > 0).
    pub zscore_threshold: Option<f64>,
    /// Numeric columns to analyse for outliers.
    /// Defaults to the entity's predefined numeric fields when omitted.
    /// Must be from the entity's allowed numeric-field list.
    pub numeric_fields: Option<Vec<String>>,
    /// Columns whose values are hashed together for duplicate detection.
    /// Defaults to the entity's predefined stable-key fields when omitted.
    /// Must be from the entity's allowed hash-field list.
    pub hash_fields: Option<Vec<String>>,
}

/// A single data-quality anomaly raised during a scan.
#[derive(Serialize, Deserialize, Clone)]
pub struct Anomaly {
    /// Check that raised this anomaly:
    /// `"missing_fields"` | `"outliers"` | `"duplicates"`
    pub check: String,
    /// Primary-key value of the offending record.
    pub record_id: i64,
    /// Column that triggered the anomaly (absent for duplicate checks).
    pub field: Option<String>,
    /// Human-readable description of the issue.
    pub detail: String,
    /// Severity classification: `"low"` | `"medium"` | `"high"`
    pub severity: String,
    /// Populated only for `"outliers"` — the computed z-score.
    pub score: Option<f64>,
}

/// Configuration actually used during the scan (defaults applied).
#[derive(Serialize, Clone)]
pub struct ScanConfig {
    pub checks_run: Vec<String>,
    pub zscore_threshold: f64,
    pub numeric_fields: Vec<String>,
    pub hash_fields: Vec<String>,
}

/// Full scan result, including every anomaly found.
#[derive(Serialize)]
pub struct ScanReport {
    pub id: i64,
    pub entity: String,
    pub config: ScanConfig,
    pub total_records: i64,
    pub anomaly_count: i64,
    pub anomalies: Vec<Anomaly>,
    pub created_by: i64,
    pub created_at: String,
}

/// Lightweight listing entry — no anomaly details.
#[derive(Serialize)]
pub struct ScanSummary {
    pub id: i64,
    pub entity: String,
    pub checks_run: Vec<String>,
    pub total_records: i64,
    pub anomaly_count: i64,
    pub created_by: i64,
    pub created_at: String,
}

/// Query parameters for `GET /data-quality/scans`.
#[derive(FromForm)]
pub struct ScanListQuery {
    /// Filter by entity type (e.g. `"invoices"`).
    pub entity: Option<String>,
    /// Maximum results to return (default: 20, max: 100).
    pub limit: Option<u64>,
}
