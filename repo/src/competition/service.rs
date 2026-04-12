use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionError, TransactionTrait,
};

use crate::audit;
use crate::entity::{enums::EventStatus, event, event_asset_binding, ruleset_version};
use crate::errors::{AppError, AppResult};

use super::{
    CreateEventRequest, CreateRulesetRequest, EventResponse, RollbackRulesetRequest,
    RulesetVersionResponse, UpdateEventRequest,
};

// ── Conversion helpers ────────────────────────────────────────────────────────

fn status_str(s: &EventStatus) -> &'static str {
    match s {
        EventStatus::Draft => "draft",
        EventStatus::Published => "published",
        EventStatus::InProgress => "in_progress",
        EventStatus::Completed => "completed",
        EventStatus::Cancelled => "cancelled",
    }
}

fn parse_status(s: &str) -> AppResult<EventStatus> {
    match s {
        "draft" => Ok(EventStatus::Draft),
        "published" => Ok(EventStatus::Published),
        "in_progress" => Ok(EventStatus::InProgress),
        "completed" => Ok(EventStatus::Completed),
        "cancelled" => Ok(EventStatus::Cancelled),
        _ => Err(AppError::BadRequest(format!("Unknown event status '{s}'"))),
    }
}

fn parse_datetime(s: &str) -> AppResult<chrono::DateTime<Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| AppError::BadRequest(format!("Invalid datetime '{s}'; expected RFC 3339")))
}

/// Minimal SemVer validation: three dot-separated non-negative integers with
/// an optional pre-release identifier (e.g. `1.2.3-rollback`, `0.5.0-rb`).
fn validate_semver(v: &str) -> AppResult<()> {
    // Strip optional pre-release (`-…`) and build metadata (`+…`) before
    // validating the numeric core so that versions like "1.2.3-rollback"
    // are accepted.
    let core = v.split('-').next().unwrap_or(v);
    let core = core.split('+').next().unwrap_or(core);
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok()) {
        Ok(())
    } else {
        Err(AppError::BadRequest(format!(
            "'{v}' is not a valid semantic version (expected X.Y.Z)"
        )))
    }
}

/// Treat an empty-string as a request to clear a nullable field.
fn coerce_optional_text(v: Option<String>) -> Option<String> {
    v.and_then(|s| if s.is_empty() { None } else { Some(s) })
}

fn to_event_response(model: event::Model, asset_ids: Vec<i64>) -> EventResponse {
    EventResponse {
        id: model.id,
        name: model.name,
        description: model.description,
        schedule_group: model.schedule_group,
        venue_identifier: model.venue_identifier,
        status: status_str(&model.status).to_owned(),
        is_championship_class: model.is_championship_class,
        published_version_id: model.published_version_id,
        asset_ids,
        created_by: model.created_by,
        created_at: model.created_at.to_rfc3339(),
        updated_at: model.updated_at.to_rfc3339(),
    }
}

fn to_ruleset_response(model: ruleset_version::Model) -> RulesetVersionResponse {
    let is_rollback = model.rollback_of.is_some();
    RulesetVersionResponse {
        id: model.id,
        semantic_version: model.semantic_version,
        description: model.description,
        effective_at: model.effective_at.to_rfc3339(),
        created_by: model.created_by,
        rollback_of: model.rollback_of,
        is_rollback,
        created_at: model.created_at.to_rfc3339(),
    }
}

/// Load all asset IDs bound to a given event (separate indexed query).
async fn load_asset_ids(
    conn: &impl sea_orm::ConnectionTrait,
    event_id: i64,
) -> Result<Vec<i64>, sea_orm::DbErr> {
    Ok(event_asset_binding::Entity::find()
        .filter(event_asset_binding::Column::EventId.eq(event_id))
        .all(conn)
        .await?
        .into_iter()
        .map(|b| b.asset_id)
        .collect())
}

// ── Transaction error adapter ─────────────────────────────────────────────────

fn tx_err(e: TransactionError<AppError>) -> AppError {
    match e {
        TransactionError::Transaction(app_err) => app_err,
        TransactionError::Connection(db_err) => AppError::Internal(db_err.to_string()),
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Event operations
// ══════════════════════════════════════════════════════════════════════════════

/// Create a new event in `draft` status and bind the supplied equipment assets.
pub async fn create_event(
    conn: &DatabaseConnection,
    user_id: i64,
    req: CreateEventRequest,
) -> AppResult<EventResponse> {
    let now = Utc::now();

    let (event_model, asset_ids) = conn
        .transaction::<_, (event::Model, Vec<i64>), AppError>(|txn| {
            let req = req.clone();
            Box::pin(async move {
                let event_model = event::ActiveModel {
                    name: Set(req.name),
                    description: Set(req.description),
                    schedule_group: Set(req.schedule_group),
                    venue_identifier: Set(req.venue_identifier),
                    is_championship_class: Set(req.is_championship_class),
                    status: Set(EventStatus::Draft),
                    published_version_id: Set(None),
                    created_by: Set(user_id),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

                for asset_id in &req.asset_ids {
                    event_asset_binding::ActiveModel {
                        event_id: Set(event_model.id),
                        asset_id: Set(*asset_id),
                        bound_at: Set(now),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;
                }

                Ok((event_model, req.asset_ids))
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        event_id = event_model.id,
        user_id,
        "competition.event_created"
    );
    let resp = to_event_response(event_model, asset_ids);
    audit::service::append(
        conn,
        user_id,
        "event.created",
        "event",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({}),
    )
    .await?;
    Ok(resp)
}

/// Get a single event by ID, including its asset bindings.
pub async fn get_event(conn: &DatabaseConnection, event_id: i64) -> AppResult<EventResponse> {
    let event_model = event::Entity::find_by_id(event_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))?;

    let asset_ids = load_asset_ids(conn, event_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(to_event_response(event_model, asset_ids))
}

/// List events with optional status and schedule_group filters.
pub async fn list_events(
    conn: &DatabaseConnection,
    status: Option<&str>,
    schedule_group: Option<&str>,
) -> AppResult<Vec<EventResponse>> {
    let mut query = event::Entity::find().order_by_desc(event::Column::CreatedAt);

    if let Some(s) = status {
        query = query.filter(event::Column::Status.eq(parse_status(s)?));
    }
    if let Some(g) = schedule_group {
        query = query.filter(event::Column::ScheduleGroup.eq(g));
    }

    let events = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut responses = Vec::with_capacity(events.len());
    for model in events {
        let asset_ids = load_asset_ids(conn, model.id)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;
        responses.push(to_event_response(model, asset_ids));
    }
    Ok(responses)
}

/// Partially update a draft event.  Only fields present in the request body are
/// written; the rest keep their current values.
///
/// **Constraint:** only `draft` events can be updated; returns 409 otherwise.
pub async fn update_event(
    conn: &DatabaseConnection,
    event_id: i64,
    user_id: i64,
    req: UpdateEventRequest,
) -> AppResult<EventResponse> {
    let existing = event::Entity::find_by_id(event_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))?;

    if existing.status != EventStatus::Draft {
        return Err(AppError::Conflict(format!(
            "Event {event_id} has status '{}' and cannot be modified; \
             only draft events are editable",
            status_str(&existing.status)
        )));
    }

    let now = Utc::now();

    let (updated_model, new_asset_ids) = conn
        .transaction::<_, (event::Model, Vec<i64>), AppError>(|txn| {
            let req = req.clone();
            let existing = existing.clone();
            Box::pin(async move {
                let mut active: event::ActiveModel = existing.into();

                if let Some(name) = req.name {
                    active.name = Set(name);
                }
                if let Some(desc) = req.description {
                    active.description = Set(coerce_optional_text(Some(desc)));
                }
                if let Some(grp) = req.schedule_group {
                    active.schedule_group = Set(coerce_optional_text(Some(grp)));
                }
                if let Some(venue) = req.venue_identifier {
                    active.venue_identifier = Set(coerce_optional_text(Some(venue)));
                }
                if let Some(champ) = req.is_championship_class {
                    active.is_championship_class = Set(champ);
                }
                active.updated_at = Set(now);

                let updated_model = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                // Replace asset bindings only when the caller explicitly provides them.
                let new_asset_ids = if let Some(ids) = req.asset_ids {
                    event_asset_binding::Entity::delete_many()
                        .filter(event_asset_binding::Column::EventId.eq(event_id))
                        .exec(txn)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?;

                    for asset_id in &ids {
                        event_asset_binding::ActiveModel {
                            event_id: Set(event_id),
                            asset_id: Set(*asset_id),
                            bound_at: Set(now),
                            ..Default::default()
                        }
                        .insert(txn)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?;
                    }
                    ids
                } else {
                    load_asset_ids(txn, event_id)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?
                };

                Ok((updated_model, new_asset_ids))
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(event_id, user_id, "competition.event_updated");
    let resp = to_event_response(updated_model, new_asset_ids);
    audit::service::append(
        conn,
        user_id,
        "event.updated",
        "event",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({}),
    )
    .await?;
    Ok(resp)
}

/// Publish a draft event by binding it to an existing ruleset version.
/// Transitions status from `draft` → `published`.  Published events are
/// immutable — their configuration is frozen at this point.
pub async fn publish_event(
    conn: &DatabaseConnection,
    event_id: i64,
    user_id: i64,
    ruleset_version_id: i64,
) -> AppResult<EventResponse> {
    let event_model = event::Entity::find_by_id(event_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Event {event_id} not found")))?;

    if event_model.status != EventStatus::Draft {
        return Err(AppError::Conflict(format!(
            "Event {event_id} is already '{}'; only draft events can be published",
            status_str(&event_model.status)
        )));
    }

    // Validate the ruleset version exists.
    ruleset_version::Entity::find_by_id(ruleset_version_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| {
            AppError::NotFound(format!("Ruleset version {ruleset_version_id} not found"))
        })?;

    let mut active: event::ActiveModel = event_model.into();
    active.status = Set(EventStatus::Published);
    active.published_version_id = Set(Some(ruleset_version_id));
    active.updated_at = Set(Utc::now());

    let updated = active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let asset_ids = load_asset_ids(conn, event_id)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        event_id,
        user_id,
        ruleset_version_id,
        "competition.event_published"
    );
    let resp = to_event_response(updated, asset_ids);
    audit::service::append(
        conn,
        user_id,
        "event.published",
        "event",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "previous_status":    "draft",
            "ruleset_version_id": ruleset_version_id,
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Ruleset version operations
// ══════════════════════════════════════════════════════════════════════════════

/// Publish a new immutable ruleset version.
///
/// Once created, a ruleset version **cannot be modified**.  To supersede it,
/// either publish a newer version or issue a rollback (which also creates a new
/// version, preserving the full audit chain).
pub async fn create_ruleset_version(
    conn: &DatabaseConnection,
    user_id: i64,
    req: CreateRulesetRequest,
) -> AppResult<RulesetVersionResponse> {
    validate_semver(&req.semantic_version)?;
    let effective_at = parse_datetime(&req.effective_at)?;

    // Check uniqueness before attempting insert for a clear error message.
    let exists = ruleset_version::Entity::find()
        .filter(ruleset_version::Column::SemanticVersion.eq(&req.semantic_version))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some();

    if exists {
        return Err(AppError::Conflict(format!(
            "Ruleset version '{}' already exists",
            req.semantic_version
        )));
    }

    let model = ruleset_version::ActiveModel {
        semantic_version: Set(req.semantic_version.clone()),
        description: Set(req.description),
        effective_at: Set(effective_at),
        created_by: Set(user_id),
        rollback_of: Set(None),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        version = %req.semantic_version,
        user_id,
        "competition.ruleset_published"
    );
    let resp = to_ruleset_response(model);
    audit::service::append(
        conn,
        user_id,
        "ruleset.created",
        "ruleset_version",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({}),
    )
    .await?;
    Ok(resp)
}

/// Get a single ruleset version by ID.
pub async fn get_ruleset_version(
    conn: &DatabaseConnection,
    version_id: i64,
) -> AppResult<RulesetVersionResponse> {
    let model = ruleset_version::Entity::find_by_id(version_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Ruleset version {version_id} not found")))?;

    Ok(to_ruleset_response(model))
}

/// List all ruleset versions ordered by creation time (newest first).
pub async fn list_ruleset_versions(
    conn: &DatabaseConnection,
) -> AppResult<Vec<RulesetVersionResponse>> {
    let models = ruleset_version::Entity::find()
        .order_by_desc(ruleset_version::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(models.into_iter().map(to_ruleset_response).collect())
}

/// Roll back a ruleset by creating a new version that supersedes the given one.
///
/// The `rollback_of` field on the new version points to the version being
/// superseded, preserving the full audit chain.  The superseded version is
/// **not modified** (immutability guarantee).
pub async fn rollback_ruleset(
    conn: &DatabaseConnection,
    version_id: i64,
    user_id: i64,
    req: RollbackRulesetRequest,
) -> AppResult<RulesetVersionResponse> {
    validate_semver(&req.new_semantic_version)?;
    let effective_at = parse_datetime(&req.effective_at)?;

    // Verify the version to rollback exists — provides a meaningful 404.
    ruleset_version::Entity::find_by_id(version_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| {
            AppError::NotFound(format!(
                "Ruleset version {version_id} not found — cannot rollback"
            ))
        })?;

    // New version must not already exist.
    let conflict = ruleset_version::Entity::find()
        .filter(ruleset_version::Column::SemanticVersion.eq(&req.new_semantic_version))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some();

    if conflict {
        return Err(AppError::Conflict(format!(
            "Ruleset version '{}' already exists",
            req.new_semantic_version
        )));
    }

    let new_model = ruleset_version::ActiveModel {
        semantic_version: Set(req.new_semantic_version.clone()),
        description: Set(req.description),
        effective_at: Set(effective_at),
        created_by: Set(user_id),
        // Audit trail: this version was created to supersede version_id.
        rollback_of: Set(Some(version_id)),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        new_version   = %req.new_semantic_version,
        rollback_of   = version_id,
        user_id,
        "competition.ruleset_rolled_back"
    );
    let resp = to_ruleset_response(new_model);
    audit::service::append(
        conn,
        user_id,
        "ruleset.rolled_back",
        "ruleset_version",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({ "rollback_of": version_id }),
    )
    .await?;
    Ok(resp)
}
