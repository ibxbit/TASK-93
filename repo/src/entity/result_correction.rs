use sea_orm::entity::prelude::*;

use super::enums::{CorrectionStatus, ResultUnit};

/// A versioned correction request against a submitted result.
///
/// Each row is **immutable after insertion** — corrections are never updated
/// in place.  The full audit chain is the ordered sequence of rows per
/// `result_id`.  Only one correction per result may be in `pending` state
/// at any time (enforced at the service layer).
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "result_corrections")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub result_id: i64,
    /// Proposed replacement value (never applied to `results` directly).
    pub corrected_value: f64,
    pub corrected_unit: ResultUnit,
    pub requested_by: i64,
    pub reason: Option<String>,
    /// Lifecycle: pending → approved | rejected.
    pub status: CorrectionStatus,
    /// Set when status transitions out of `pending`.
    pub resolved_by: Option<i64>,
    pub resolved_at: Option<DateTimeUtc>,
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
