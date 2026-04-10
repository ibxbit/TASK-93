use chrono::DateTime;
use sea_orm::entity::prelude::*;

/// Read model for the unified append-only audit trail.
///
/// The table has database-level immutability enforced by BEFORE UPDATE/DELETE
/// triggers; no application-layer path performs updates or deletes on it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// The authenticated user who triggered the operation.
    pub actor_id: i64,
    /// Dot-namespaced action identifier, e.g. `"invoice.created"`.
    pub action: String,
    /// Domain object type, e.g. `"invoice"`, `"vehicle"`, `"result"`.
    pub entity_type: String,
    /// Primary key of the affected domain object.
    pub entity_id: i64,
    /// Full JSON snapshot of the entity state *after* the operation.
    pub snapshot: String,
    /// Supplementary key-value context (reason, previous status, etc.).
    pub metadata: String,
    pub created_at: DateTime<chrono::Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
