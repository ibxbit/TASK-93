use sea_orm::entity::prelude::*;

/// Append-only audit trail for every change to a vehicle record.
///
/// Each row holds a full JSON snapshot of the vehicle state *after* the
/// change.  The sequence of rows per `vehicle_id` is the complete version
/// history.  Rows are never updated or deleted.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "vehicle_audit_log")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: i64,
    pub vehicle_id: i64,
    pub changed_by: i64,
    /// created | updated | status_changed | mileage_updated | title_transferred
    pub change_type: String,
    /// Full JSON snapshot of the vehicle row after this change.
    #[sea_orm(column_type = "Text")]
    pub snapshot: String,
    pub changed_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::vehicle::Entity",
        from = "Column::VehicleId",
        to = "super::vehicle::Column::Id",
        on_delete = "Cascade"
    )]
    Vehicle,
}

impl Related<super::vehicle::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Vehicle.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
