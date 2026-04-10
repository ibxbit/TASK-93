use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;

use super::enums::{AdjustmentType, PricingModel};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "invoice_lines")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub invoice_id: i64,
    pub description: String,
    pub pricing_model: PricingModel,
    /// Fractional quantities supported (e.g. 2.5 hours for per_duration lines).
    pub quantity: f64,
    pub unit_price: Decimal,
    /// NULL means no adjustment applied to this line.
    pub adjustment_type: Option<AdjustmentType>,
    /// When `true`, `adjustment_value` is a percentage of the base line amount
    /// (0–30 for discounts, unlimited for surcharges).
    /// When `false`, `adjustment_value` is a fixed dollar amount.
    pub adjustment_is_percentage: bool,
    /// Percentage value (0–30 for discounts) or fixed dollar amount,
    /// depending on `adjustment_is_percentage`.
    pub adjustment_value: Option<Decimal>,
    pub line_total: Decimal,
    /// When `true`, this line is a refund credit (line_total may be negative).
    /// Refund lines are inserted automatically when a payment refund is approved.
    pub is_refund: bool,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::invoice::Entity",
        from = "Column::InvoiceId",
        to = "super::invoice::Column::Id",
        on_delete = "Cascade"
    )]
    Invoice,
}

impl Related<super::invoice::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invoice.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
