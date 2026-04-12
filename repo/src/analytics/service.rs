use rust_decimal::prelude::FromPrimitive;
use chrono::{NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    DbBackend, EntityTrait, QueryFilter, QueryOrder, Statement,
};

use crate::entity::{
    metric_definition::{self as metric_entity, ActiveModel as MetricActiveModel},
    metric_definition_version::{self as version_entity, ActiveModel as VersionActiveModel},
};
use crate::errors::{AppError, AppResult};

use super::{
    CreateMetricRequest, ExportQuery, FunnelQuery, FunnelResponse, FunnelStep,
    MetricDetailResponse, MetricResponse, MetricVersionResponse, RetentionPeriod, RetentionQuery,
    RetentionResponse, RetentionRow, TrendPoint, TrendQuery, TrendResponse, UpdateMetricRequest,
};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn metric_to_response(m: &metric_entity::Model) -> MetricResponse {
    MetricResponse {
        id: m.id,
        name: m.name.clone(),
        definition: m.definition.clone(),
        unit: m.unit.clone(),
        category: m.category.clone(),
        version: m.version,
        owner_id: m.owner_id,
        is_active: m.is_active,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    }
}

fn version_to_response(v: &version_entity::Model) -> MetricVersionResponse {
    MetricVersionResponse {
        id: v.id,
        metric_id: v.metric_id,
        version: v.version,
        definition: v.definition.clone(),
        changed_by: v.changed_by,
        change_reason: v.change_reason.clone(),
        created_at: v.created_at.to_rfc3339(),
    }
}

fn validate_category(s: &str) -> AppResult<()> {
    if matches!(s, "financial" | "operational" | "results" | "assets") {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "Unknown category '{s}'. Valid: financial, operational, results, assets"
        )))
    }
}

fn parse_date_param(s: &str, field: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("Invalid {field} '{s}'; expected YYYY-MM-DD")))
}

fn bucket_fmt(bucket: &str) -> AppResult<&'static str> {
    match bucket {
        "day" => Ok("%Y-%m-%d"),
        "week" => Ok("%Y-%W"),
        "month" | "" => Ok("%Y-%m"),
        _ => Err(AppError::BadRequest(format!(
            "Unknown bucket '{bucket}'. Valid: day, week, month"
        ))),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Metric catalog CRUD
// ══════════════════════════════════════════════════════════════════════════════

pub async fn create_metric(
    conn: &DatabaseConnection,
    user_id: i64,
    req: CreateMetricRequest,
) -> AppResult<MetricResponse> {
    if req.name.trim().is_empty() {
        return Err(AppError::BadRequest("name is required".into()));
    }
    if req.definition.trim().is_empty() {
        return Err(AppError::BadRequest("definition is required".into()));
    }
    validate_category(&req.category)?;

    let now = Utc::now();

    let metric = MetricActiveModel {
        name: Set(req.name.trim().to_owned()),
        definition: Set(req.definition.trim().to_owned()),
        unit: Set(req.unit.clone()),
        category: Set(req.category.clone()),
        version: Set(1),
        owner_id: Set(req.owner_id),
        is_active: Set(true),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            AppError::Conflict(format!("Metric '{}' already exists", req.name))
        } else {
            AppError::Internal(msg)
        }
    })?;

    // Seed the initial version record.
    VersionActiveModel {
        metric_id: Set(metric.id),
        version: Set(1),
        definition: Set(metric.definition.clone()),
        changed_by: Set(Some(user_id)),
        change_reason: Set(Some("Initial version".into())),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        metric_id = metric.id,
        name = %metric.name,
        user_id,
        "analytics.metric_created"
    );

    Ok(metric_to_response(&metric))
}

pub async fn list_metrics(conn: &DatabaseConnection) -> AppResult<Vec<MetricResponse>> {
    let metrics = metric_entity::Entity::find()
        .order_by_asc(metric_entity::Column::Name)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(metrics.iter().map(metric_to_response).collect())
}

pub async fn get_metric(conn: &DatabaseConnection, id: i64) -> AppResult<MetricDetailResponse> {
    let metric = metric_entity::Entity::find_by_id(id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Metric {id} not found")))?;

    let versions = version_entity::Entity::find()
        .filter(version_entity::Column::MetricId.eq(id))
        .order_by_asc(version_entity::Column::Version)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(MetricDetailResponse {
        metric: metric_to_response(&metric),
        history: versions.iter().map(version_to_response).collect(),
    })
}

pub async fn update_metric(
    conn: &DatabaseConnection,
    id: i64,
    user_id: i64,
    req: UpdateMetricRequest,
) -> AppResult<MetricResponse> {
    if req.definition.trim().is_empty() {
        return Err(AppError::BadRequest("definition is required".into()));
    }
    if req.change_reason.trim().is_empty() {
        return Err(AppError::BadRequest("change_reason is required".into()));
    }

    let metric = metric_entity::Entity::find_by_id(id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Metric {id} not found")))?;

    if !metric.is_active {
        return Err(AppError::Conflict(format!("Metric {id} is inactive")));
    }

    let new_version = metric.version + 1;
    let now = Utc::now();

    // Insert version snapshot first.
    VersionActiveModel {
        metric_id: Set(id),
        version: Set(new_version),
        definition: Set(req.definition.trim().to_owned()),
        changed_by: Set(Some(user_id)),
        change_reason: Set(Some(req.change_reason.trim().to_owned())),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut active: MetricActiveModel = metric.into();
    active.definition = Set(req.definition.trim().to_owned());
    active.unit = Set(req.unit);
    active.version = Set(new_version);
    active.updated_at = Set(now);

    let updated = active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        metric_id = id,
        new_version,
        user_id,
        "analytics.metric_updated"
    );

    Ok(metric_to_response(&updated))
}

// ══════════════════════════════════════════════════════════════════════════════
// Trend analysis
// ══════════════════════════════════════════════════════════════════════════════

pub async fn compute_trends(conn: &DatabaseConnection, q: TrendQuery) -> AppResult<TrendResponse> {
    let start = q
        .start_date
        .as_deref()
        .map(|s| parse_date_param(s, "start_date"))
        .transpose()?
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());

    let end = q
        .end_date
        .as_deref()
        .map(|s| parse_date_param(s, "end_date"))
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());

    let bucket = q.bucket.as_deref().unwrap_or("month");
    let fmt = bucket_fmt(bucket)?;

    let data = match q.metric.as_str() {
        "invoice_revenue" => trend_invoice_revenue(conn, fmt, &start, &end).await?,
        "invoice_count" => trend_invoice_count(conn, fmt, &start, &end).await?,
        "payment_volume" => trend_payment_volume(conn, fmt, &start, &end).await?,
        "payment_count" => trend_payment_count(conn, fmt, &start, &end).await?,
        "results_submitted" => {
            trend_results_submitted(
                conn,
                fmt,
                &start,
                &end,
                q.event_id,
                q.is_championship_class,
                q.venue.as_deref(),
            )
            .await?
        }
        "active_assets" => {
            trend_active_assets(conn, fmt, &start, &end, q.asset_category.as_deref()).await?
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown metric '{other}'. Valid: invoice_revenue, invoice_count, \
                 payment_volume, payment_count, results_submitted, active_assets"
            )));
        }
    };

    Ok(TrendResponse {
        metric: q.metric.clone(),
        bucket_size: bucket.to_owned(),
        start_date: start.to_string(),
        end_date: end.to_string(),
        data,
    })
}

async fn raw_trend(
    conn: &DatabaseConnection,
    sql: &str,
    params: Vec<sea_orm::Value>,
) -> AppResult<Vec<TrendPoint>> {
    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, sql, params);
    let rows = conn
        .query_all(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    rows.iter()
        .map(|row| {
            let bucket: String = row
                .try_get("", "bucket")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let value: f64 = row.try_get("", "value").unwrap_or(0.0);
            Ok(TrendPoint {
                bucket,
                value: Decimal::from_f64(value)
                    .unwrap_or(Decimal::ZERO)
                    .round_dp(4)
                    .to_string(),
            })
        })
        .collect()
}

async fn trend_invoice_revenue(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<TrendPoint>> {
    let sql = format!(
        "SELECT strftime(?, CAST(issue_date AS TEXT)) AS bucket, \
                COALESCE(SUM(CAST(total AS REAL)), 0.0) AS value \
         FROM invoices \
         WHERE issue_date >= ? AND issue_date <= ? \
         GROUP BY bucket ORDER BY bucket"
    );
    raw_trend(
        conn,
        &sql,
        vec![fmt.into(), start.to_string().into(), end.to_string().into()],
    )
    .await
}

async fn trend_invoice_count(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<TrendPoint>> {
    let sql = "SELECT strftime(?, CAST(issue_date AS TEXT)) AS bucket, \
                      CAST(COUNT(*) AS REAL) AS value \
               FROM invoices \
               WHERE issue_date >= ? AND issue_date <= ? \
               GROUP BY bucket ORDER BY bucket";
    raw_trend(
        conn,
        sql,
        vec![fmt.into(), start.to_string().into(), end.to_string().into()],
    )
    .await
}

async fn trend_payment_volume(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<TrendPoint>> {
    let sql = "SELECT strftime(?, received_at) AS bucket, \
                      COALESCE(SUM(CAST(amount AS REAL)), 0.0) AS value \
               FROM payment_entries \
               WHERE status = 'active' \
                 AND date(received_at) >= ? AND date(received_at) <= ? \
               GROUP BY bucket ORDER BY bucket";
    raw_trend(
        conn,
        sql,
        vec![fmt.into(), start.to_string().into(), end.to_string().into()],
    )
    .await
}

async fn trend_payment_count(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<TrendPoint>> {
    let sql = "SELECT strftime(?, received_at) AS bucket, \
                      CAST(COUNT(*) AS REAL) AS value \
               FROM payment_entries \
               WHERE status = 'active' \
                 AND date(received_at) >= ? AND date(received_at) <= ? \
               GROUP BY bucket ORDER BY bucket";
    raw_trend(
        conn,
        sql,
        vec![fmt.into(), start.to_string().into(), end.to_string().into()],
    )
    .await
}

async fn trend_results_submitted(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
    event_id: Option<i64>,
    is_championship: Option<bool>,
    venue: Option<&str>,
) -> AppResult<Vec<TrendPoint>> {
    let mut where_clauses = vec![
        "date(r.created_at) >= ?".to_owned(),
        "date(r.created_at) <= ?".to_owned(),
    ];
    let mut params: Vec<sea_orm::Value> = vec![start.to_string().into(), end.to_string().into()];

    if let Some(eid) = event_id {
        where_clauses.push("r.event_id = ?".into());
        params.push(eid.into());
    }
    if let Some(champ) = is_championship {
        where_clauses.push("e.is_championship_class = ?".into());
        params.push((champ as i32).into());
    }
    if let Some(v) = venue {
        where_clauses.push("e.venue_identifier = ?".into());
        params.push(v.to_owned().into());
    }

    let where_str = where_clauses.join(" AND ");
    let sql = format!(
        "SELECT strftime(?, r.created_at) AS bucket, \
                CAST(COUNT(*) AS REAL) AS value \
         FROM results r \
         JOIN events e ON r.event_id = e.id \
         WHERE {where_str} \
         GROUP BY bucket ORDER BY bucket"
    );

    let mut full_params = vec![sea_orm::Value::String(Some(Box::new(fmt.to_owned())))];
    full_params.extend(params);

    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, &sql, full_params);
    let rows = conn
        .query_all(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    rows.iter()
        .map(|row| {
            let bucket: String = row
                .try_get("", "bucket")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let value: f64 = row.try_get("", "value").unwrap_or(0.0);
            Ok(TrendPoint {
                bucket,
                value: (value as i64).to_string(),
            })
        })
        .collect()
}

async fn trend_active_assets(
    conn: &DatabaseConnection,
    fmt: &str,
    start: &NaiveDate,
    end: &NaiveDate,
    category: Option<&str>,
) -> AppResult<Vec<TrendPoint>> {
    let mut where_clauses = vec![
        "status != 'retired'".to_owned(),
        "date(created_at) >= ?".to_owned(),
        "date(created_at) <= ?".to_owned(),
    ];
    let mut params: Vec<sea_orm::Value> = vec![start.to_string().into(), end.to_string().into()];

    if let Some(cat) = category {
        where_clauses.push("category = ?".into());
        params.push(cat.to_owned().into());
    }

    let where_str = where_clauses.join(" AND ");
    let sql = format!(
        "SELECT strftime(?, created_at) AS bucket, \
                CAST(COUNT(*) AS REAL) AS value \
         FROM assets \
         WHERE {where_str} \
         GROUP BY bucket ORDER BY bucket"
    );

    let mut full_params = vec![sea_orm::Value::String(Some(Box::new(fmt.to_owned())))];
    full_params.extend(params);

    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, &sql, full_params);
    let rows = conn
        .query_all(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    rows.iter()
        .map(|row| {
            let bucket: String = row
                .try_get("", "bucket")
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let value: f64 = row.try_get("", "value").unwrap_or(0.0);
            Ok(TrendPoint {
                bucket,
                value: (value as i64).to_string(),
            })
        })
        .collect()
}

// ══════════════════════════════════════════════════════════════════════════════
// Funnel analysis
// ══════════════════════════════════════════════════════════════════════════════

pub async fn compute_funnel(
    conn: &DatabaseConnection,
    q: FunnelQuery,
) -> AppResult<FunnelResponse> {
    let start = q
        .start_date
        .as_deref()
        .map(|s| parse_date_param(s, "start_date"))
        .transpose()?
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2020, 1, 1).unwrap());

    let end = q
        .end_date
        .as_deref()
        .map(|s| parse_date_param(s, "end_date"))
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());

    let steps = match q.funnel_type.as_str() {
        "invoice_lifecycle" => funnel_invoice_lifecycle(conn, &start, &end).await?,
        "result_review" => {
            funnel_result_review(
                conn,
                &start,
                &end,
                q.event_id,
                q.is_championship_class,
                q.venue.as_deref(),
            )
            .await?
        }
        "refund_approval" => funnel_refund_approval(conn, &start, &end).await?,
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown funnel_type '{other}'. Valid: invoice_lifecycle, result_review, refund_approval"
            )));
        }
    };

    Ok(FunnelResponse {
        funnel_type: q.funnel_type,
        start_date: start.to_string(),
        end_date: end.to_string(),
        steps,
    })
}

fn build_funnel_steps(raw: Vec<(&'static str, i64)>) -> Vec<FunnelStep> {
    let baseline = raw.first().map(|(_, c)| *c).unwrap_or(1).max(1);
    let mut prev = baseline;
    raw.into_iter()
        .map(|(stage, count)| {
            let rate = Decimal::from(count) / Decimal::from(baseline);
            let drop = prev - count;
            prev = count;
            FunnelStep {
                stage: stage.to_owned(),
                count,
                conversion_rate: rate.round_dp(4).to_string(),
                drop_off: drop.max(0),
            }
        })
        .collect()
}

async fn query_count(
    conn: &DatabaseConnection,
    sql: &str,
    params: Vec<sea_orm::Value>,
) -> AppResult<i64> {
    let stmt = Statement::from_sql_and_values(DbBackend::Sqlite, sql, params);
    let rows = conn
        .query_all(stmt)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;
    if let Some(row) = rows.first() {
        let n: i64 = row.try_get("", "n").unwrap_or(0);
        Ok(n)
    } else {
        Ok(0)
    }
}

async fn funnel_invoice_lifecycle(
    conn: &DatabaseConnection,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<FunnelStep>> {
    let base_params: Vec<sea_orm::Value> = vec![start.to_string().into(), end.to_string().into()];
    let date_filter = "issue_date >= ? AND issue_date <= ?";

    let total = query_count(
        conn,
        &format!("SELECT COUNT(*) AS n FROM invoices WHERE {date_filter}"),
        base_params.clone(),
    )
    .await?;

    let issued = query_count(
        conn,
        &format!("SELECT COUNT(*) AS n FROM invoices WHERE status != 'draft' AND {date_filter}"),
        base_params.clone(),
    )
    .await?;

    let paid = query_count(
        conn,
        &format!("SELECT COUNT(*) AS n FROM invoices WHERE status = 'paid' AND {date_filter}"),
        base_params.clone(),
    )
    .await?;

    Ok(build_funnel_steps(vec![
        ("draft_created", total),
        ("issued", issued),
        ("paid", paid),
    ]))
}

async fn funnel_result_review(
    conn: &DatabaseConnection,
    start: &NaiveDate,
    end: &NaiveDate,
    event_id: Option<i64>,
    is_championship: Option<bool>,
    venue: Option<&str>,
) -> AppResult<Vec<FunnelStep>> {
    let mut join = String::new();
    let mut extra_where = String::new();
    let mut base_params: Vec<sea_orm::Value> =
        vec![start.to_string().into(), end.to_string().into()];

    if event_id.is_some() || is_championship.is_some() || venue.is_some() {
        join = " JOIN events e ON r.event_id = e.id".into();
        if let Some(eid) = event_id {
            extra_where.push_str(" AND r.event_id = ?");
            base_params.push(eid.into());
        }
        if let Some(champ) = is_championship {
            extra_where.push_str(" AND e.is_championship_class = ?");
            base_params.push((champ as i32).into());
        }
        if let Some(v) = venue {
            extra_where.push_str(" AND e.venue_identifier = ?");
            base_params.push(v.to_owned().into());
        }
    }

    let date_filter = "date(r.created_at) >= ? AND date(r.created_at) <= ?";

    let submitted = query_count(
        conn,
        &format!(
            "SELECT COUNT(DISTINCT r.id) AS n FROM results r{join} WHERE {date_filter}{extra_where}"
        ),
        base_params.clone(),
    )
    .await?;

    let reviewed = query_count(
        conn,
        &format!(
            "SELECT COUNT(DISTINCT r.id) AS n \
             FROM results r{join} \
             JOIN result_reviews rv ON rv.result_id = r.id \
             WHERE {date_filter}{extra_where}"
        ),
        base_params.clone(),
    )
    .await?;

    let approved = query_count(
        conn,
        &format!(
            "SELECT COUNT(DISTINCT r.id) AS n FROM results r{join} \
             WHERE r.reviewed_state = 'approved' AND {date_filter}{extra_where}"
        ),
        base_params.clone(),
    )
    .await?;

    let rejected = query_count(
        conn,
        &format!(
            "SELECT COUNT(DISTINCT r.id) AS n FROM results r{join} \
             WHERE r.reviewed_state = 'rejected' AND {date_filter}{extra_where}"
        ),
        base_params,
    )
    .await?;

    Ok(build_funnel_steps(vec![
        ("submitted", submitted),
        ("reviewed", reviewed),
        ("approved", approved),
        ("rejected", rejected),
    ]))
}

async fn funnel_refund_approval(
    conn: &DatabaseConnection,
    start: &NaiveDate,
    end: &NaiveDate,
) -> AppResult<Vec<FunnelStep>> {
    let params: Vec<sea_orm::Value> = vec![start.to_string().into(), end.to_string().into()];
    let date_filter = "date(created_at) >= ? AND date(created_at) <= ?";

    let requested = query_count(
        conn,
        &format!("SELECT COUNT(*) AS n FROM payment_refunds WHERE {date_filter}"),
        params.clone(),
    )
    .await?;

    let escalated = query_count(
        conn,
        &format!(
            "SELECT COUNT(*) AS n FROM payment_refunds \
             WHERE status IN ('pending_auditor','approved') AND {date_filter}"
        ),
        params.clone(),
    )
    .await?;

    let approved = query_count(
        conn,
        &format!(
            "SELECT COUNT(*) AS n FROM payment_refunds WHERE status = 'approved' AND {date_filter}"
        ),
        params,
    )
    .await?;

    Ok(build_funnel_steps(vec![
        ("requested", requested),
        ("finance_approved", escalated),
        ("fully_approved", approved),
    ]))
}

// ══════════════════════════════════════════════════════════════════════════════
// Retention analysis
// ══════════════════════════════════════════════════════════════════════════════

pub async fn compute_retention(
    conn: &DatabaseConnection,
    q: RetentionQuery,
) -> AppResult<RetentionResponse> {
    let start = q
        .start_date
        .as_deref()
        .map(|s| parse_date_param(s, "start_date"))
        .transpose()?
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(2023, 1, 1).unwrap());

    let end = q
        .end_date
        .as_deref()
        .map(|s| parse_date_param(s, "end_date"))
        .transpose()?
        .unwrap_or_else(|| Utc::now().date_naive());

    let periods = q.periods.unwrap_or(3).clamp(1, 6);

    let rows = match q.retention_type.as_str() {
        "event_participation" => retention_event_participation(conn, &start, &end, periods).await?,
        "invoice_payment" => retention_invoice_payment(conn, &start, &end, periods).await?,
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown retention_type '{other}'. Valid: event_participation, invoice_payment"
            )));
        }
    };

    Ok(RetentionResponse {
        retention_type: q.retention_type,
        start_date: start.to_string(),
        end_date: end.to_string(),
        rows,
    })
}

/// Retention: of participants who had their first result in cohort month X,
/// how many returned in months X+1 … X+N?
async fn retention_event_participation(
    conn: &DatabaseConnection,
    start: &NaiveDate,
    end: &NaiveDate,
    periods: i32,
) -> AppResult<Vec<RetentionRow>> {
    // Step 1: Get cohort sizes — first month each participant appeared.
    let cohort_sql = "
        SELECT strftime('%Y-%m', e.created_at) AS cohort, COUNT(DISTINCT r.participant_id) AS cohort_size
        FROM results r
        JOIN events e ON r.event_id = e.id
        WHERE date(e.created_at) >= ? AND date(e.created_at) <= ?
        GROUP BY strftime('%Y-%m', e.created_at)
        ORDER BY cohort";

    let cohort_rows = conn
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            cohort_sql,
            vec![start.to_string().into(), end.to_string().into()],
        ))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut result_rows = Vec::new();

    for crow in &cohort_rows {
        let cohort: String = crow.try_get("", "cohort").unwrap_or_default();
        let cohort_size: i64 = crow.try_get("", "cohort_size").unwrap_or(0);

        if cohort.is_empty() || cohort_size == 0 {
            continue;
        }

        let mut period_data = Vec::new();

        for p in 1..=periods {
            // Count participants from this cohort who also appeared in cohort+p months.
            let _ret_sql = format!(
                "SELECT COUNT(DISTINCT r2.participant_id) AS n
                 FROM results r1
                 JOIN events e1 ON r1.event_id = e1.id
                 JOIN results r2 ON r1.participant_id = r2.participant_id
                 JOIN events e2 ON r2.event_id = e2.id
                 WHERE strftime('%Y-%m', MIN(e1.created_at)) = ?
                   AND strftime('%Y-%m', e2.created_at) = strftime('%Y-%m', date(? || '-01', '+{p} month'))
                 GROUP BY r1.participant_id
                 HAVING COUNT(*) > 0"
            );

            // Simpler approach: count participants in that cohort who appeared in the Nth month.
            let simple_sql = format!(
                "SELECT COUNT(DISTINCT r.participant_id) AS n
                 FROM results r
                 JOIN events e ON r.event_id = e.id
                 WHERE strftime('%Y-%m', e.created_at) = strftime('%Y-%m', date(? || '-01', '+{p} month'))
                   AND r.participant_id IN (
                     SELECT DISTINCT r2.participant_id
                     FROM results r2
                     JOIN events e2 ON r2.event_id = e2.id
                     WHERE strftime('%Y-%m', e2.created_at) = ?
                   )"
            );

            let count = query_count(
                conn,
                &simple_sql,
                vec![cohort.clone().into(), cohort.clone().into()],
            )
            .await
            .unwrap_or(0);

            let rate = if cohort_size > 0 {
                (Decimal::from(count) / Decimal::from(cohort_size))
                    .round_dp(4)
                    .to_string()
            } else {
                "0.0000".into()
            };

            period_data.push(RetentionPeriod {
                period: p,
                count,
                rate,
            });
        }

        result_rows.push(RetentionRow {
            cohort,
            cohort_size,
            periods: period_data,
        });
    }

    Ok(result_rows)
}

/// Retention: of invoices issued in cohort month X, how many were paid
/// within N×30-day windows?
async fn retention_invoice_payment(
    conn: &DatabaseConnection,
    start: &NaiveDate,
    end: &NaiveDate,
    periods: i32,
) -> AppResult<Vec<RetentionRow>> {
    let cohort_sql = "
        SELECT strftime('%Y-%m', CAST(issue_date AS TEXT)) AS cohort, COUNT(*) AS cohort_size
        FROM invoices
        WHERE issue_date >= ? AND issue_date <= ?
        GROUP BY cohort
        ORDER BY cohort";

    let cohort_rows = conn
        .query_all(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            cohort_sql,
            vec![start.to_string().into(), end.to_string().into()],
        ))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut result_rows = Vec::new();

    for crow in &cohort_rows {
        let cohort: String = crow.try_get("", "cohort").unwrap_or_default();
        let cohort_size: i64 = crow.try_get("", "cohort_size").unwrap_or(0);

        if cohort.is_empty() || cohort_size == 0 {
            continue;
        }

        let mut period_data = Vec::new();

        // Period N = paid within N*30 days of issue.
        for p in 1..=periods {
            let days = p * 30;
            let paid_sql = format!(
                "SELECT COUNT(*) AS n
                 FROM invoices
                 WHERE strftime('%Y-%m', CAST(issue_date AS TEXT)) = ?
                   AND status = 'paid'
                   AND julianday(updated_at) - julianday(CAST(issue_date AS TEXT)) <= {days}"
            );

            let count = query_count(conn, &paid_sql, vec![cohort.clone().into()])
                .await
                .unwrap_or(0);

            let rate = if cohort_size > 0 {
                (Decimal::from(count) / Decimal::from(cohort_size))
                    .round_dp(4)
                    .to_string()
            } else {
                "0.0000".into()
            };

            period_data.push(RetentionPeriod {
                period: p,
                rate,
                count,
            });
        }

        result_rows.push(RetentionRow {
            cohort,
            cohort_size,
            periods: period_data,
        });
    }

    Ok(result_rows)
}

// ══════════════════════════════════════════════════════════════════════════════
// CSV export
// ══════════════════════════════════════════════════════════════════════════════

pub async fn export_csv(conn: &DatabaseConnection, q: ExportQuery) -> AppResult<String> {
    let mut out = String::new();

    match q.report_type.as_str() {
        "trends" => {
            let metric = q.metric.as_deref().unwrap_or("invoice_revenue").to_owned();
            let tq = TrendQuery {
                metric: metric.clone(),
                start_date: q.start_date,
                end_date: q.end_date,
                bucket: q.bucket,
                event_id: q.event_id,
                is_championship_class: q.is_championship_class,
                venue: q.venue,
                asset_category: q.asset_category,
            };
            let resp = compute_trends(conn, tq).await?;
            out.push_str("bucket,metric,value\n");
            for pt in &resp.data {
                out.push_str(&format!(
                    "{},{},{}\n",
                    escape_csv(&pt.bucket),
                    escape_csv(&metric),
                    escape_csv(&pt.value)
                ));
            }
        }
        "funnel" => {
            let funnel_type = q
                .funnel_type
                .as_deref()
                .unwrap_or("invoice_lifecycle")
                .to_owned();
            let fq = FunnelQuery {
                funnel_type: funnel_type.clone(),
                start_date: q.start_date,
                end_date: q.end_date,
                event_id: q.event_id,
                is_championship_class: q.is_championship_class,
                venue: q.venue,
            };
            let resp = compute_funnel(conn, fq).await?;
            out.push_str("stage,count,conversion_rate,drop_off\n");
            for step in &resp.steps {
                out.push_str(&format!(
                    "{},{},{},{}\n",
                    escape_csv(&step.stage),
                    step.count,
                    escape_csv(&step.conversion_rate),
                    step.drop_off
                ));
            }
        }
        "retention" => {
            let retention_type = q
                .retention_type
                .as_deref()
                .unwrap_or("invoice_payment")
                .to_owned();
            let rq = RetentionQuery {
                retention_type: retention_type.clone(),
                start_date: q.start_date,
                end_date: q.end_date,
                periods: q.periods,
            };
            let resp = compute_retention(conn, rq).await?;

            // Build dynamic header based on max periods.
            let max_periods = resp.rows.iter().map(|r| r.periods.len()).max().unwrap_or(0);
            let mut header = "cohort,cohort_size".to_owned();
            for p in 1..=max_periods {
                header.push_str(&format!(",period_{p}_count,period_{p}_rate"));
            }
            out.push_str(&header);
            out.push('\n');

            for row in &resp.rows {
                let mut line = format!("{},{}", escape_csv(&row.cohort), row.cohort_size);
                for p in &row.periods {
                    line.push_str(&format!(",{},{}", p.count, escape_csv(&p.rate)));
                }
                out.push_str(&line);
                out.push('\n');
            }
        }
        other => {
            return Err(AppError::BadRequest(format!(
                "Unknown report_type '{other}'. Valid: trends, funnel, retention"
            )));
        }
    }

    Ok(out)
}

/// Minimal CSV escaping: wrap field in quotes if it contains comma, quote, or newline.
fn escape_csv(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}
