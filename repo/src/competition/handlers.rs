use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::errors::AppResult;
use crate::rbac::guards::{RequireEventsRead, RequireEventsWrite};

use super::{
    service, CreateEventRequest, CreateRulesetRequest, EventFilterQuery, EventResponse,
    PublishEventRequest, RollbackRulesetRequest, RulesetVersionResponse, UpdateEventRequest,
};

// ══════════════════════════════════════════════════════════════════════════════
// Event endpoints
// ══════════════════════════════════════════════════════════════════════════════

/// Create a new event in `draft` status.
/// Optionally binds equipment assets and sets a venue identifier.
///
/// **Required permission:** `events:write`
#[post("/events", data = "<body>")]
pub async fn create_event(
    guard: RequireEventsWrite,
    body: Json<CreateEventRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<EventResponse>> {
    let resp = service::create_event(conn.inner(), guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

/// Retrieve a single event by ID, including its bound asset IDs.
///
/// **Required permission:** `events:read`
#[get("/events/<id>")]
pub async fn get_event(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<EventResponse>> {
    Ok(Json(service::get_event(conn.inner(), id).await?))
}

/// List events with optional filters.
///
/// **Query params:**
/// - `status` — draft | published | in_progress | completed | cancelled
/// - `schedule_group` — exact match on the schedule group name
///
/// **Required permission:** `events:read`
#[get("/events?<filter..>")]
pub async fn list_events(
    _guard: RequireEventsRead,
    filter: EventFilterQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<EventResponse>>> {
    let resp = service::list_events(
        conn.inner(),
        filter.status.as_deref(),
        filter.schedule_group.as_deref(),
    )
    .await?;
    Ok(Json(resp))
}

/// Partially update a draft event.
///
/// Only fields present in the JSON body are written.  Send an empty string
/// `""` to clear a nullable text field.  Asset bindings are fully replaced
/// if `asset_ids` is provided.
///
/// Returns 409 if the event is not in `draft` status.
///
/// **Required permission:** `events:write`
#[put("/events/<id>", data = "<body>")]
pub async fn update_event(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<UpdateEventRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<EventResponse>> {
    let resp = service::update_event(conn.inner(), id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

/// Publish a draft event, locking its configuration and associating it with
/// an immutable ruleset version.  Transitions status `draft` → `published`.
///
/// After publishing the event cannot be updated; its venue, equipment bindings,
/// and ruleset version are frozen for audit purposes.
///
/// **Required permission:** `events:write`
#[post("/events/<id>/publish", data = "<body>")]
pub async fn publish_event(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<PublishEventRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<EventResponse>> {
    let resp =
        service::publish_event(conn.inner(), id, guard.0.user_id, body.ruleset_version_id).await?;
    Ok(Json(resp))
}

// ══════════════════════════════════════════════════════════════════════════════
// Ruleset version endpoints
// ══════════════════════════════════════════════════════════════════════════════

/// Publish a new immutable ruleset version.
///
/// Ruleset versions are append-only — once created, their content (semantic
/// version, effective timestamp, description) **cannot be changed**.  To
/// supersede a version, publish a newer one or issue a rollback.
///
/// **Required permission:** `events:write`
#[post("/rulesets", data = "<body>")]
pub async fn create_ruleset(
    guard: RequireEventsWrite,
    body: Json<CreateRulesetRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<RulesetVersionResponse>> {
    let resp =
        service::create_ruleset_version(conn.inner(), guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

/// Get a single ruleset version by ID.
///
/// **Required permission:** `events:read`
#[get("/rulesets/<id>")]
pub async fn get_ruleset(
    _guard: RequireEventsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<RulesetVersionResponse>> {
    Ok(Json(service::get_ruleset_version(conn.inner(), id).await?))
}

/// List all ruleset versions, newest first.
///
/// **Required permission:** `events:read`
#[get("/rulesets")]
pub async fn list_rulesets(
    _guard: RequireEventsRead,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<RulesetVersionResponse>>> {
    Ok(Json(service::list_ruleset_versions(conn.inner()).await?))
}

/// Roll back a ruleset by creating a new version that supersedes the given one.
///
/// The superseded version is **not modified** — the new version's `rollback_of`
/// field points to it, preserving the complete audit chain.
///
/// **Required permission:** `events:write`
#[post("/rulesets/<id>/rollback", data = "<body>")]
pub async fn rollback_ruleset(
    guard: RequireEventsWrite,
    id: i64,
    body: Json<RollbackRulesetRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<RulesetVersionResponse>> {
    let resp =
        service::rollback_ruleset(conn.inner(), id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}
