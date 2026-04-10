use rust_decimal::Decimal;
use sea_orm::entity::prelude::*;

use super::enums::{AssetCategory, AssetStatus};

/// A capitalised asset in the asset register.
///
/// Financial fields use `Decimal` for precision.  Depreciation (straight-line)
/// is computed on read from `procurement_cost`, `procurement_date`, and
/// `useful_life_months`; no pre-computed value is stored.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "assets")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    /// Human-readable unique code (e.g. "ASSET-2024-001").
    #[sea_orm(unique)]
    pub asset_code: String,
    pub category: AssetCategory,
    pub brand: String,
    pub model: String,
    pub serial_number: Option<String>,
    /// Operational lifecycle state.
    pub status: AssetStatus,
    /// User who owns this asset (legal/financial owner).
    pub owner_id: Option<i64>,
    /// User responsible for day-to-day custody.
    pub responsible_person_id: Option<i64>,
    /// Original acquisition cost.
    pub procurement_cost: Option<Decimal>,
    /// ISO 8601 date string (YYYY-MM-DD) of acquisition.
    pub procurement_date: Option<String>,
    /// Straight-line depreciation period — total months to full write-off.
    pub useful_life_months: Option<i32>,
    pub notes: Option<String>,
    pub created_at: DateTimeUtc,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::vehicle::Entity")]
    Vehicles,
    #[sea_orm(has_many = "super::asset_audit_log::Entity")]
    AuditLog,
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicles.def()
    }
}

impl Related<super::asset_audit_log::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::AuditLog.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
