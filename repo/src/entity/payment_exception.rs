use sea_orm::entity::prelude::*;

use super::enums::ExceptionType;

/// Immutable record of an exception raised against a payment entry.
///
/// Raising an exception (void / reversal / dispute) simultaneously updates the
/// parent `payment_entry.status` and inserts one row here for the audit trail.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "payment_exceptions")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub payment_id: i64,
    pub exception_type: ExceptionType,
    pub reason: String,
    pub raised_by: i64,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::payment_entry::Entity",
        from = "Column::PaymentId",
        to = "super::payment_entry::Column::Id",
        on_delete = "Restrict"
    )]
    PaymentEntry,
}

impl Related<super::payment_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PaymentEntry.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
