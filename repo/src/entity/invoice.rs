use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;

use super::enums::InvoiceStatus;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "invoices")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Human-readable reference (e.g. "INV-2024-0042").
    #[sea_orm(unique)]
    pub invoice_no: String,
    /// Customer or vendor name / identifier.
    pub counterparty: String,
    pub issue_date: Date,
    /// Tax rate as a decimal fraction (e.g. 0.1000 = 10 %).
    pub tax_rate: Decimal,
    pub subtotal: Decimal,
    /// Computed: subtotal × tax_rate, rounded to 4 dp.
    pub tax: Decimal,
    /// "percentage" | "fixed_amount" | NULL (no discount).
    pub discount_type: Option<String>,
    /// Raw discount input: percentage points (0–30) or dollar amount.
    pub discount_value: Option<Decimal>,
    /// Computed discount in dollars; capped at $500 per invoice.
    pub discount_amount: Decimal,
    /// subtotal + tax - discount_amount.
    pub total: Decimal,
    pub status: InvoiceStatus,
    pub created_by: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::invoice_line::Entity")]
    Lines,
    #[sea_orm(has_many = "super::payment_entry::Entity")]
    Payments,
}

impl Related<super::invoice_line::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Lines.def()
    }
}

impl Related<super::payment_entry::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Payments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
