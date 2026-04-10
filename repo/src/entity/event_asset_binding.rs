use sea_orm::entity::prelude::*;

/// Junction table binding equipment assets to an event.
/// A binding is created when equipment is assigned for an event and
/// removed when it is unassigned.  Bindings are locked once the event
/// transitions to `published` status.
#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "event_asset_bindings")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub event_id: i64,
    pub asset_id: i64,
    pub bound_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::event::Entity",
        from = "Column::EventId",
        to = "super::event::Column::Id",
        on_delete = "Cascade"
    )]
    Event,
    #[sea_orm(
        belongs_to = "super::asset::Entity",
        from = "Column::AssetId",
        to = "super::asset::Column::Id",
        on_delete = "Restrict"
    )]
    Asset,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl Related<super::asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Asset.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
