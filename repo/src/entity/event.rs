use sea_orm::entity::prelude::*;

use super::enums::EventStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "events")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    /// Logical series / round grouping (e.g. "2024 Championship Round 1–5").
    pub schedule_group: Option<String>,
    /// Free-text venue identifier (e.g. "VENUE-MONZA-01").
    pub venue_identifier: Option<String>,
    pub status: EventStatus,
    /// FK → ruleset_versions.id; set when the event is published.
    pub published_version_id: Option<i64>,
    /// When true, ≥ 2 referee reviews are required before a result can be
    /// auto-approved (without arbitration).
    pub is_championship_class: bool,
    pub created_by: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::ruleset_version::Entity",
        from = "Column::PublishedVersionId",
        to = "super::ruleset_version::Column::Id"
    )]
    RulesetVersion,
    #[sea_orm(has_many = "super::result::Entity")]
    Results,
    #[sea_orm(has_many = "super::event_asset_binding::Entity")]
    AssetBindings,
}

impl Related<super::ruleset_version::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::RulesetVersion.def()
    }
}

impl Related<super::result::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Results.def()
    }
}

impl Related<super::event_asset_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AssetBindings.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
