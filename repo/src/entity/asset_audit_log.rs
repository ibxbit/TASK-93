use sea_orm::entity::prelude::*;

/// Append-only audit trail for every state change on an asset.
///
/// Each row is a snapshot of the full asset state *after* the change.
/// The ordered sequence of rows per `asset_id` constitutes the complete
/// version history.  Rows are never updated or deleted.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "asset_audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub asset_id: i64,
    /// User who triggered the change.
    pub changed_by: i64,
    /// High-level description of what changed.
    /// Values: created | updated | status_changed | owner_changed | imported
    pub change_type: String,
    /// Full JSON snapshot of the asset row immediately after the change.
    #[sea_orm(column_type = "Text")]
    pub snapshot: String,
    pub changed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::asset::Entity",
        from = "Column::AssetId",
        to = "super::asset::Column::Id",
        on_delete = "Cascade"
    )]
    Asset,
}

impl Related<super::asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Asset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
