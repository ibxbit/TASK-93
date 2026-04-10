pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Request types ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct CreateEventRequest {
    pub name: String,
    pub description: Option<String>,
    pub schedule_group: Option<String>,
    pub venue_identifier: Option<String>,
    /// When true, results in this event require ≥ 2 referee reviews before
    /// auto-approval (championship rules).
    #[serde(default)]
    pub is_championship_class: bool,
    /// Asset IDs to bind to this event as equipment.
    #[serde(default)]
    pub asset_ids: Vec<i64>,
}

/// All fields optional — only provided fields overwrite the stored value.
/// Send an empty string `""` to clear a nullable text field (sets it to NULL).
#[derive(Deserialize, Clone, Default)]
pub struct UpdateEventRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub schedule_group: Option<String>,
    pub venue_identifier: Option<String>,
    /// If present, updates the championship-class flag.
    pub is_championship_class: Option<bool>,
    /// If present, replaces the full asset binding list for this event.
    pub asset_ids: Option<Vec<i64>>,
}

#[derive(Deserialize)]
pub struct PublishEventRequest {
    /// The ruleset version to associate when publishing the event.
    pub ruleset_version_id: i64,
}

/// Request body for creating (publishing) a new immutable ruleset version.
#[derive(Deserialize, Clone)]
pub struct CreateRulesetRequest {
    /// Semantic version string, e.g. "2.1.0".
    pub semantic_version: String,
    pub description: Option<String>,
    /// RFC 3339 timestamp of when this version becomes effective.
    pub effective_at: String,
}

/// Request body for rolling back to a previous ruleset state.
/// Creates a new version whose `rollback_of` points to the given version.
#[derive(Deserialize, Clone)]
pub struct RollbackRulesetRequest {
    pub new_semantic_version: String,
    pub description: Option<String>,
    pub effective_at: String,
}

// ── Response types ────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct EventResponse {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub schedule_group: Option<String>,
    pub venue_identifier: Option<String>,
    pub status: String,
    pub is_championship_class: bool,
    pub published_version_id: Option<i64>,
    /// IDs of equipment assets bound to this event.
    pub asset_ids: Vec<i64>,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Clone)]
pub struct RulesetVersionResponse {
    pub id: i64,
    pub semantic_version: String,
    pub description: Option<String>,
    pub effective_at: String,
    pub created_by: i64,
    /// ID of the version this row supersedes in a rollback; NULL for forward releases.
    pub rollback_of: Option<i64>,
    pub is_rollback: bool,
    pub created_at: String,
}

// ── Query filter ──────────────────────────────────────────────────────────────

#[derive(rocket::FromForm)]
pub struct EventFilterQuery {
    pub status: Option<String>,
    pub schedule_group: Option<String>,
}
