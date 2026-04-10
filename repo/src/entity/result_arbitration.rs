use sea_orm::entity::prelude::*;

use super::enums::ReviewDecision;

/// An Event Director's binding arbitration decision on a result.
///
/// One arbitration per result (enforced by UNIQUE on result_id).  Once
/// recorded, the row is **never modified** — the decision overrides whatever
/// referee reviews preceded it.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "result_arbitrations")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub result_id: i64,
    /// The Event Director (or Administrator) who issued the decision.
    pub arbitrated_by: i64,
    pub decision: ReviewDecision,
    pub comment: Option<String>,
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
