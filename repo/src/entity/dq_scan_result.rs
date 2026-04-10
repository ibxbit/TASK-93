use sea_orm::entity::prelude::*;

/// Persisted record of a single data-quality scan run.
///
/// Array-valued config fields (`checks_run`, `numeric_fields`, `hash_fields`)
/// are stored as JSON arrays.  The `report` column stores the full
/// `Vec<Anomaly>` serialised to JSON.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "dq_scan_results")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Entity type that was scanned (e.g. `"invoices"`).
    pub entity: String,
    /// JSON array of check names that were run.
    pub checks_run: String,
    /// Z-score threshold used for outlier detection.
    pub zscore_threshold: f64,
    /// JSON array of numeric column names analysed for outliers.
    pub numeric_fields: String,
    /// JSON array of column names hashed for duplicate detection.
    pub hash_fields: String,
    /// Total number of records in the scanned table at scan time.
    pub total_records: i64,
    /// Number of anomalies found across all checks.
    pub anomaly_count: i64,
    /// JSON blob — serialised `Vec<Anomaly>`.
    pub report: String,
    pub created_by: i64,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
