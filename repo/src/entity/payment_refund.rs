use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;

use super::enums::RefundStatus;

/// A refund request attached to an individual payment entry.
///
/// Approval workflow:
/// - Finance Clerk approves: `pending_finance` → `approved` (< $1,000)
///                           `pending_finance` → `pending_auditor` (≥ $1,000)
/// - Auditor approves:       `pending_auditor` → `approved`
/// - Either role rejects:    any pending state → `rejected`
///
/// On final approval an `invoice_line` with `is_refund = true` is inserted and
/// `invoice_line_id` is populated here.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "payment_refunds")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub payment_id: i64,
    /// Populated once the refund is approved and the invoice line is inserted.
    pub invoice_line_id: Option<i64>,
    pub amount: Decimal,
    pub reason: String,
    pub status: RefundStatus,
    pub requested_by: i64,
    pub finance_approved_by: Option<i64>,
    pub auditor_approved_by: Option<i64>,
    pub rejected_by: Option<i64>,
    pub rejection_reason: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
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
