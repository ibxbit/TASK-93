use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbBackend, EntityTrait,
    QueryFilter, QueryOrder, Statement,
};
use std::collections::HashMap;

use crate::entity::dq_scan_result::{self as scan_entity, ActiveModel as ScanActiveModel};
use crate::errors::{AppError, AppResult};

use super::{Anomaly, ScanConfig, ScanListQuery, ScanReport, ScanRequest, ScanSummary};

// ── Per-entity metadata ───────────────────────────────────────────────────────

struct EntityMeta {
    /// SQL table name.
    table: &'static str,
    /// (column_name, SQL fragment that is TRUE when the field is missing/empty).
    mandatory: &'static [(&'static str, &'static str)],
    /// Columns valid for z-score outlier analysis.
    allowed_numeric: &'static [&'static str],
    /// Columns used for outlier analysis when the caller omits `numeric_fields`.
    default_numeric: &'static [&'static str],
    /// Columns valid as hash keys for duplicate detection.
    allowed_hash: &'static [&'static str],
    /// Columns hashed for duplicates when the caller omits `hash_fields`.
    default_hash: &'static [&'static str],
}

static ENTITIES: &[EntityMeta] = &[
    EntityMeta {
        table: "events",
        mandatory: &[
            ("name", "TRIM(name) = ''"),
            (
                "description",
                "description IS NULL OR TRIM(description) = ''",
            ),
            (
                "venue_identifier",
                "venue_identifier IS NULL OR TRIM(venue_identifier) = ''",
            ),
            (
                "schedule_group",
                "schedule_group IS NULL OR TRIM(schedule_group) = ''",
            ),
        ],
        allowed_numeric: &[],
        default_numeric: &[],
        allowed_hash: &["name", "venue_identifier", "schedule_group", "status"],
        default_hash: &["name", "venue_identifier"],
    },
    EntityMeta {
        table: "assets",
        mandatory: &[
            ("asset_code", "TRIM(asset_code) = ''"),
            ("brand", "TRIM(brand) = ''"),
            ("model", "TRIM(model) = ''"),
            (
                "serial_number",
                "serial_number IS NULL OR TRIM(serial_number) = ''",
            ),
            ("procurement_cost", "procurement_cost IS NULL"),
            (
                "procurement_date",
                "procurement_date IS NULL OR TRIM(procurement_date) = ''",
            ),
        ],
        allowed_numeric: &["procurement_cost", "useful_life_months"],
        default_numeric: &["procurement_cost"],
        allowed_hash: &["asset_code", "brand", "model", "serial_number", "category"],
        default_hash: &["brand", "model", "serial_number"],
    },
    EntityMeta {
        table: "vehicles",
        mandatory: &[
            ("vin", "TRIM(vin) = ''"),
            ("registration_id", "TRIM(registration_id) = ''"),
            ("make", "TRIM(make) = ''"),
            ("model", "TRIM(model) = ''"),
            ("color", "color IS NULL OR TRIM(color) = ''"),
        ],
        allowed_numeric: &["year", "mileage", "title_transfer_count"],
        default_numeric: &["mileage"],
        allowed_hash: &["vin", "make", "model", "registration_id"],
        default_hash: &["vin"],
    },
    EntityMeta {
        table: "invoices",
        mandatory: &[
            ("invoice_no", "TRIM(invoice_no) = ''"),
            ("counterparty", "TRIM(counterparty) = ''"),
        ],
        allowed_numeric: &["subtotal", "tax", "total"],
        default_numeric: &["total"],
        allowed_hash: &["invoice_no", "counterparty", "status"],
        default_hash: &["invoice_no", "counterparty"],
    },
    EntityMeta {
        table: "results",
        mandatory: &[],
        allowed_numeric: &["value_numeric"],
        default_numeric: &["value_numeric"],
        allowed_hash: &["event_id", "participant_id", "attempt_no", "unit_enum"],
        default_hash: &["event_id", "participant_id", "attempt_no"],
    },
    EntityMeta {
        table: "payment_entries",
        mandatory: &[
            ("external_reference", "TRIM(external_reference) = ''"),
            ("notes", "notes IS NULL OR TRIM(notes) = ''"),
        ],
        allowed_numeric: &["amount"],
        default_numeric: &["amount"],
        allowed_hash: &["invoice_id", "external_reference", "method"],
        default_hash: &["invoice_id", "external_reference"],
    },
];

fn entity_meta(entity: &str) -> Option<&'static EntityMeta> {
    ENTITIES
        .iter()
        .find(|m| m.table == entity || (entity == "payments" && m.table == "payment_entries"))
}

// ── FNV-1a hash (no external crate required) ──────────────────────────────────

fn fnv1a_hash(data: &str) -> u64 {
    const OFFSET: u64 = 14_695_981_039_346_656_037;
    const PRIME: u64 = 1_099_511_628_211;
    let mut h = OFFSET;
    for byte in data.bytes() {
        h ^= byte as u64;
        h = h.wrapping_mul(PRIME);
    }
    h
}

// ── Statistics ────────────────────────────────────────────────────────────────

/// Returns `(mean, population_stddev)`.
/// Returns `None` when there are fewer than 2 samples or all values are identical.
fn compute_stats(values: &[f64]) -> Option<(f64, f64)> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let variance = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    let stddev = variance.sqrt();
    if stddev < f64::EPSILON {
        return None; // all identical — no outliers possible
    }
    Some((mean, stddev))
}

// ── Check implementations ─────────────────────────────────────────────────────

async fn check_missing_fields(
    conn: &DatabaseConnection,
    meta: &EntityMeta,
) -> AppResult<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    for (col, condition) in meta.mandatory {
        let sql = format!("SELECT id FROM {} WHERE {}", meta.table, condition);
        let stmt = Statement::from_string(DbBackend::Sqlite, sql);
        let rows = conn
            .query_all(stmt)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        for row in &rows {
            let record_id: i64 = row
                .try_get::<i64>("", "id")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            anomalies.push(Anomaly {
                check: "missing_fields".to_string(),
                record_id,
                field: Some(col.to_string()),
                detail: format!("Required field '{}' is null or empty", col),
                severity: "high".to_string(),
                score: None,
            });
        }
    }

    Ok(anomalies)
}

async fn check_outliers(
    conn: &DatabaseConnection,
    meta: &EntityMeta,
    fields: &[String],
    threshold: f64,
) -> AppResult<Vec<Anomaly>> {
    let mut anomalies = Vec::new();

    for field in fields {
        let sql = format!(
            "SELECT id, CAST({} AS REAL) AS val FROM {} WHERE {} IS NOT NULL",
            field, meta.table, field
        );
        let stmt = Statement::from_string(DbBackend::Sqlite, sql);
        let rows = conn
            .query_all(stmt)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        let pairs: Vec<(i64, f64)> = rows
            .iter()
            .filter_map(|row| {
                let id = row.try_get::<i64>("", "id").ok()?;
                let val = row.try_get::<f64>("", "val").ok()?;
                Some((id, val))
            })
            .collect();

        let raw_vals: Vec<f64> = pairs.iter().map(|(_, v)| *v).collect();

        if let Some((mean, stddev)) = compute_stats(&raw_vals) {
            for (record_id, val) in &pairs {
                let zscore = (val - mean).abs() / stddev;
                if zscore > threshold {
                    let severity = if zscore >= 5.0 { "high" } else { "medium" }.to_string();
                    anomalies.push(Anomaly {
                        check: "outliers".to_string(),
                        record_id: *record_id,
                        field: Some(field.clone()),
                        detail: format!(
                            "Value {:.4} deviates {:.4}σ from mean {:.4} \
                             (stddev {:.4}, threshold {:.1})",
                            val, zscore, mean, stddev, threshold
                        ),
                        severity,
                        score: Some((zscore * 1_000_000.0).round() / 1_000_000.0),
                    });
                }
            }
        }
    }

    Ok(anomalies)
}

async fn check_duplicates(
    conn: &DatabaseConnection,
    meta: &EntityMeta,
    fields: &[String],
) -> AppResult<Vec<Anomaly>> {
    // Cast every hash-key column to TEXT so all types can be fetched uniformly.
    let cast_cols: String = fields
        .iter()
        .map(|f| format!("CAST({} AS TEXT) AS {}", f, f))
        .collect::<Vec<_>>()
        .join(", ");

    let sql = format!("SELECT id, {} FROM {}", cast_cols, meta.table);
    let stmt = Statement::from_string(DbBackend::Sqlite, sql);
    let rows = conn
        .query_all(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Map FNV-1a(field_values) → [record_ids]
    let mut hash_groups: HashMap<u64, Vec<i64>> = HashMap::new();

    for row in &rows {
        let id: i64 = match row.try_get::<i64>("", "id") {
            Ok(v) => v,
            Err(_) => continue,
        };

        let mut parts = Vec::with_capacity(fields.len());
        for field in fields {
            let val = row
                .try_get::<Option<String>>("", field.as_str())
                .unwrap_or(None)
                .unwrap_or_else(|| "NULL".to_string());
            parts.push(val);
        }

        let hash = fnv1a_hash(&parts.join("|"));
        hash_groups.entry(hash).or_default().push(id);
    }

    let field_list = fields.join(", ");
    let mut anomalies = Vec::new();

    for ids in hash_groups.values().filter(|v| v.len() > 1) {
        let dup_ids: String = ids
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        for &record_id in ids {
            anomalies.push(Anomaly {
                check: "duplicates".to_string(),
                record_id,
                field: None,
                detail: format!(
                    "Duplicate group on fields [{}]: records [{}]",
                    field_list, dup_ids
                ),
                severity: "medium".to_string(),
                score: None,
            });
        }
    }

    Ok(anomalies)
}

async fn count_records(conn: &DatabaseConnection, table: &str) -> AppResult<i64> {
    let sql = format!("SELECT COUNT(*) AS cnt FROM {}", table);
    let stmt = Statement::from_string(DbBackend::Sqlite, sql);
    let row = conn
        .query_one(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("COUNT(*) returned no rows".to_string()))?;
    row.try_get::<i64>("", "cnt")
        .map_err(|e| AppError::Internal(e.to_string()))
}

// ── DB row ↔ response types ───────────────────────────────────────────────────

fn row_to_report(row: scan_entity::Model) -> AppResult<ScanReport> {
    let checks_run: Vec<String> = serde_json::from_str(&row.checks_run)
        .map_err(|e| AppError::Internal(format!("checks_run parse error: {e}")))?;
    let numeric_fields: Vec<String> = serde_json::from_str(&row.numeric_fields)
        .map_err(|e| AppError::Internal(format!("numeric_fields parse error: {e}")))?;
    let hash_fields: Vec<String> = serde_json::from_str(&row.hash_fields)
        .map_err(|e| AppError::Internal(format!("hash_fields parse error: {e}")))?;
    let anomalies: Vec<Anomaly> = serde_json::from_str(&row.report)
        .map_err(|e| AppError::Internal(format!("report parse error: {e}")))?;

    Ok(ScanReport {
        id: row.id,
        entity: row.entity,
        config: ScanConfig {
            checks_run,
            zscore_threshold: row.zscore_threshold,
            numeric_fields,
            hash_fields,
        },
        total_records: row.total_records,
        anomaly_count: row.anomaly_count,
        anomalies,
        created_by: row.created_by,
        created_at: row.created_at.to_rfc3339(),
    })
}

fn row_to_summary(row: &scan_entity::Model) -> AppResult<ScanSummary> {
    let checks_run: Vec<String> = serde_json::from_str(&row.checks_run)
        .map_err(|e| AppError::Internal(format!("checks_run parse error: {e}")))?;

    Ok(ScanSummary {
        id: row.id,
        entity: row.entity.clone(),
        checks_run,
        total_records: row.total_records,
        anomaly_count: row.anomaly_count,
        created_by: row.created_by,
        created_at: row.created_at.to_rfc3339(),
    })
}

// ── Public service functions ──────────────────────────────────────────────────

pub async fn run_scan(
    conn: &DatabaseConnection,
    user_id: i64,
    req: ScanRequest,
) -> AppResult<ScanReport> {
    // ── Validate checks list ──────────────────────────────────────────────────
    if req.checks.is_empty() {
        return Err(AppError::BadRequest(
            "At least one check must be specified in 'checks'".to_string(),
        ));
    }
    const VALID_CHECKS: &[&str] = &["missing_fields", "outliers", "duplicates"];
    for check in &req.checks {
        if !VALID_CHECKS.contains(&check.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Unknown check '{}'. Supported: [{}]",
                check,
                VALID_CHECKS.join(", ")
            )));
        }
    }

    // ── Validate z-score threshold ────────────────────────────────────────────
    let threshold = req.zscore_threshold.unwrap_or(3.0);
    if threshold <= 0.0 {
        return Err(AppError::BadRequest(
            "zscore_threshold must be > 0".to_string(),
        ));
    }

    // ── Resolve entity metadata ───────────────────────────────────────────────
    let meta = entity_meta(&req.entity).ok_or_else(|| {
        AppError::BadRequest(format!(
            "Unknown entity '{}'. \
             Supported: events, assets, vehicles, invoices, results, payments",
            req.entity
        ))
    })?;

    // ── Resolve effective field lists (apply defaults) ────────────────────────
    let numeric_fields: Vec<String> = req
        .numeric_fields
        .unwrap_or_else(|| meta.default_numeric.iter().map(|s| s.to_string()).collect());
    let hash_fields: Vec<String> = req
        .hash_fields
        .unwrap_or_else(|| meta.default_hash.iter().map(|s| s.to_string()).collect());

    // Validate caller-supplied field names against entity allow-lists.
    for field in &numeric_fields {
        if !meta.allowed_numeric.contains(&field.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Field '{}' is not valid for outlier analysis on '{}'. Allowed: [{}]",
                field,
                req.entity,
                meta.allowed_numeric.join(", ")
            )));
        }
    }
    for field in &hash_fields {
        if !meta.allowed_hash.contains(&field.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Field '{}' is not valid for duplicate detection on '{}'. Allowed: [{}]",
                field,
                req.entity,
                meta.allowed_hash.join(", ")
            )));
        }
    }

    // ── Execute checks ────────────────────────────────────────────────────────
    let total_records = count_records(conn, meta.table).await?;
    let mut anomalies: Vec<Anomaly> = Vec::new();

    for check in &req.checks {
        match check.as_str() {
            "missing_fields" => {
                anomalies.extend(check_missing_fields(conn, meta).await?);
            }
            "outliers" => {
                if !numeric_fields.is_empty() {
                    anomalies.extend(check_outliers(conn, meta, &numeric_fields, threshold).await?);
                }
            }
            "duplicates" => {
                if !hash_fields.is_empty() {
                    anomalies.extend(check_duplicates(conn, meta, &hash_fields).await?);
                }
            }
            _ => unreachable!(),
        }
    }

    // ── Persist scan result ───────────────────────────────────────────────────
    let config = ScanConfig {
        checks_run: req.checks.clone(),
        zscore_threshold: threshold,
        numeric_fields: numeric_fields.clone(),
        hash_fields: hash_fields.clone(),
    };

    let checks_json =
        serde_json::to_string(&config.checks_run).map_err(|e| AppError::Internal(e.to_string()))?;
    let numeric_json = serde_json::to_string(&config.numeric_fields)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let hash_json = serde_json::to_string(&config.hash_fields)
        .map_err(|e| AppError::Internal(e.to_string()))?;
    let report_json =
        serde_json::to_string(&anomalies).map_err(|e| AppError::Internal(e.to_string()))?;

    let saved = ScanActiveModel {
        entity: Set(req.entity.clone()),
        checks_run: Set(checks_json),
        zscore_threshold: Set(threshold),
        numeric_fields: Set(numeric_json),
        hash_fields: Set(hash_json),
        total_records: Set(total_records),
        anomaly_count: Set(anomalies.len() as i64),
        report: Set(report_json),
        created_by: Set(user_id),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(ScanReport {
        id: saved.id,
        entity: saved.entity,
        config,
        total_records: saved.total_records,
        anomaly_count: saved.anomaly_count,
        anomalies,
        created_by: saved.created_by,
        created_at: saved.created_at.to_rfc3339(),
    })
}

pub async fn list_scans(
    conn: &DatabaseConnection,
    q: ScanListQuery,
) -> AppResult<Vec<ScanSummary>> {
    let limit = q.limit.unwrap_or(20).min(100) as usize;

    let mut query = scan_entity::Entity::find().order_by_desc(scan_entity::Column::CreatedAt);

    if let Some(entity) = q.entity {
        query = query.filter(scan_entity::Column::Entity.eq(entity));
    }

    let rows = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    rows.iter().take(limit).map(row_to_summary).collect()
}

pub async fn get_scan(conn: &DatabaseConnection, id: i64) -> AppResult<ScanReport> {
    let row = scan_entity::Entity::find_by_id(id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Scan {id} not found")))?;

    row_to_report(row)
}
