use sea_orm::entity::prelude::*;

use super::enums::{ResultUnit, ReviewedState};

/// A single timed/measured attempt by one participant in one event.
/// UNIQUE constraint: (event_id, participant_id, attempt_no).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "results")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub event_id: i64,
    /// References users.id for now; will be updated to a participants table
    /// when that domain module is introduced.
    pub participant_id: i64,
    /// 1-based counter: attempt 1, 2, 3 for a participant within an event.
    pub attempt_no: i32,
    /// Raw measurement value (race time, distance, etc.).
    pub value_numeric: f64,
    pub unit_enum: ResultUnit,
    /// User who entered this result into the system.
    pub entered_by: i64,
    pub reviewed_state: ReviewedState,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
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
    #[sea_orm(has_many = "super::result_review::Entity")]
    Reviews,
}

impl Related<super::event::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Event.def()
    }
}

impl Related<super::result_review::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Reviews.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
