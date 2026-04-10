use sea_orm::entity::prelude::*;

use super::enums::VehicleLifecycleStatus;

/// A vehicle in the commercial lifecycle registry.
///
/// Invariants enforced at the service layer:
/// - `vin` is immutable after creation.
/// - `mileage` is monotonically non-decreasing.
/// - `title_transfer_count` is ≥ 0 and is incremented on every `Sold` transition.
/// - Status transitions follow a strict directed graph (see `VehicleLifecycleStatus`).
/// - Transitions to `Delisted` or `Sold` require a non-empty `status_reason`.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicles")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Optional link to the asset register; NULL if not capitalised.
    pub asset_id: Option<i64>,
    /// ISO 3779 VIN stored as AES-256-GCM ciphertext (base64 nonce || ciphertext).
    pub vin: String,
    /// Keyed FNV-1a digest of the plaintext VIN — used for SQL uniqueness checks.
    #[sea_orm(unique)]
    pub vin_hash: String,
    /// Licence plate / registration number stored as AES-256-GCM ciphertext.
    pub registration_id: String,
    pub make: String,
    pub model: String,
    /// Four-digit model year.
    pub year: i32,
    pub color: Option<String>,
    /// Odometer reading.  Must never be set to a value lower than the current one.
    pub mileage: i64,
    /// Number of ownership transfers; incremented automatically on `Sold` transition.
    pub title_transfer_count: i32,
    pub status: VehicleLifecycleStatus,
    /// Mandatory text recorded when transitioning to `Delisted` or `Sold`.
    pub status_reason: Option<String>,
    pub created_by: i64,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::asset::Entity",
        from = "Column::AssetId",
        to = "super::asset::Column::Id",
        on_delete = "SetNull"
    )]
    Asset,
    #[sea_orm(has_many = "super::vehicle_audit_log::Entity")]
    AuditLog,
}

impl Related<super::asset::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Asset.def()
    }
}

impl Related<super::vehicle_audit_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuditLog.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
