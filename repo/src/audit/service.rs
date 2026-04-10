use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, ConnectionTrait, DatabaseConnection,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};

use crate::entity::audit_log::{self as audit_entity, ActiveModel as AuditActiveModel};
use crate::errors::{AppError, AppResult};

use super::{AuditLogEntry, AuditLogQuery};

// ── Write path ────────────────────────────────────────────────────────────────

/// Append an immutable entry to the unified audit trail.
///
/// This function accepts any `ConnectionTrait` implementation so it can be
/// called both inside an existing transaction and directly on the pool
/// connection.  The database-level triggers on `audit_log` guarantee
/// immutability regardless of which path is used.
pub async fn append(
    conn: &impl ConnectionTrait,
    actor_id: i64,
    action: &str,
    entity_type: &str,
    entity_id: i64,
    snapshot: serde_json::Value,
    metadata: serde_json::Value,
) -> AppResult<()> {
    let snapshot_str = serde_json::to_string(&snapshot).unwrap_or_else(|_| "{}".to_owned());
    let metadata_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_owned());

    AuditActiveModel {
        actor_id: Set(actor_id),
        action: Set(action.to_owned()),
        entity_type: Set(entity_type.to_owned()),
        entity_id: Set(entity_id),
        snapshot: Set(snapshot_str),
        metadata: Set(metadata_str),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(format!("audit write failed: {e}")))?;

    Ok(())
}

// ── Read path ─────────────────────────────────────────────────────────────────

/// Query the audit trail with optional filters, returning entries newest-first.
///
/// Limits are capped at 500 rows to protect against large result sets.
pub async fn list(conn: &DatabaseConnection, q: AuditLogQuery) -> AppResult<Vec<AuditLogEntry>> {
    let limit = q.limit.unwrap_or(100).min(500);
    let offset = q.offset.unwrap_or(0);

    let mut query = audit_entity::Entity::find()
        .order_by_desc(audit_entity::Column::CreatedAt)
        .limit(limit)
        .offset(offset);

    if let Some(actor_id) = q.actor_id {
        query = query.filter(audit_entity::Column::ActorId.eq(actor_id));
    }
    if let Some(ref et) = q.entity_type {
        query = query.filter(audit_entity::Column::EntityType.eq(et.as_str()));
    }
    if let Some(eid) = q.entity_id {
        query = query.filter(audit_entity::Column::EntityId.eq(eid));
    }
    if let Some(ref action) = q.action {
        query = query.filter(audit_entity::Column::Action.eq(action.as_str()));
    }
    if let Some(ref from) = q.from {
        query = query.filter(
            audit_entity::Column::CreatedAt.gte(
                chrono::DateTime::parse_from_rfc3339(from)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|_| {
                        AppError::BadRequest(format!("Invalid 'from' timestamp: {from}"))
                    })?,
            ),
        );
    }
    if let Some(ref to) = q.to {
        query = query.filter(
            audit_entity::Column::CreatedAt.lte(
                chrono::DateTime::parse_from_rfc3339(to)
                    .map(|dt| dt.with_timezone(&chrono::Utc))
                    .map_err(|_| AppError::BadRequest(format!("Invalid 'to' timestamp: {to}")))?,
            ),
        );
    }

    let models = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    models.iter().map(to_entry).collect()
}

/// Retrieve a single audit log entry by its primary key.
pub async fn get(conn: &DatabaseConnection, id: i64) -> AppResult<AuditLogEntry> {
    let model = audit_entity::Entity::find_by_id(id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Audit log entry {id} not found")))?;

    to_entry(&model)
}

// ── Conversion helper ─────────────────────────────────────────────────────────

fn to_entry(m: &audit_entity::Model) -> AppResult<AuditLogEntry> {
    let snapshot =
        serde_json::from_str(&m.snapshot).unwrap_or(serde_json::Value::Object(Default::default()));
    let metadata =
        serde_json::from_str(&m.metadata).unwrap_or(serde_json::Value::Object(Default::default()));

    Ok(AuditLogEntry {
        id: m.id,
        actor_id: m.actor_id,
        action: m.action.clone(),
        entity_type: m.entity_type.clone(),
        entity_id: m.entity_id,
        snapshot,
        metadata,
        created_at: m.created_at.to_rfc3339(),
    })
}
