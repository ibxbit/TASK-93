use sea_orm::entity::prelude::*;

use super::enums::ReviewDecision;

/// A referee's formal decision on a submitted result.
/// UNIQUE constraint: (result_id, referee_id) — one decision per referee per result.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "result_reviews")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub result_id: i64,
    /// The referee submitting this decision.
    pub referee_id: i64,
    pub decision: ReviewDecision,
    pub comment: Option<String>,
    pub reviewed_at: DateTimeUtc,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::result::Entity",
        from = "Column::ResultId",
        to = "super::result::Column::Id",
        on_delete = "Cascade"
    )]
    Result,
}

impl Related<super::result::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Result.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
