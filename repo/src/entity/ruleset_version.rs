use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "ruleset_versions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// SemVer string (e.g. "2.1.0").  Unique across all versions.
    #[sea_orm(unique)]
    pub semantic_version: String,
    pub description: Option<String>,
    pub effective_at: DateTimeUtc,
    pub created_by: i64,
    /// ID of the version being rolled back; NULL for forward releases.
    pub rollback_of: Option<i64>,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// Events published under this version.
    #[sea_orm(has_many = "super::event::Entity")]
    Events,
    /// Self-reference: the version this row is rolling back.
    #[sea_orm(belongs_to = "Entity", from = "Column::RollbackOf", to = "Column::Id")]
    RollbackSource,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Events.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
