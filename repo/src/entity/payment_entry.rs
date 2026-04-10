use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;

use super::enums::{PaymentEntryStatus, PaymentMethod};

/// An individual payment recorded against an invoice.
/// `external_reference` is UNIQUE to prevent duplicate processing of the same
/// transaction from an external payment processor.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "payment_entries")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub invoice_id: i64,
    pub method: PaymentMethod,
    pub amount: Decimal,
    pub received_at: DateTimeUtc,
    /// Encrypted idempotency key from the payment processor.
    /// Stored as AES-256-GCM ciphertext (base64-encoded nonce || ciphertext).
    pub external_reference: String,
    /// Keyed FNV-1a digest of the plaintext external_reference.
    /// Used for SQL equality lookups in place of the encrypted column.
    #[sea_orm(unique)]
    pub reference_hash: String,
    /// User who recorded this payment in the system.
    pub recorded_by: i64,
    pub notes: Option<String>,
    /// Lifecycle status — exceptions (void, reversal, dispute) update this field.
    pub status: PaymentEntryStatus,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::invoice::Entity",
        from = "Column::InvoiceId",
        to = "super::invoice::Column::Id",
        on_delete = "Restrict"
    )]
    Invoice,
}

impl Related<super::invoice::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Invoice.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
