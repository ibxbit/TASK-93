use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CHANGE_TYPE_CHECK: &str =
    "change_type IN ('created','updated','status_changed','mileage_updated','title_transferred')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(VehicleAuditLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(VehicleAuditLog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(VehicleAuditLog::VehicleId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VehicleAuditLog::ChangedBy)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(VehicleAuditLog::ChangeType)
                            .string_len(24)
                            .not_null(),
                    )
                    .col(ColumnDef::new(VehicleAuditLog::Snapshot).text().not_null())
                    .col(
                        ColumnDef::new(VehicleAuditLog::ChangedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(CHANGE_TYPE_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_audit_log_vehicle_id")
                            .from(VehicleAuditLog::Table, VehicleAuditLog::VehicleId)
                            .to(Vehicles::Table, Vehicles::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicle_audit_log_changed_by")
                            .from(VehicleAuditLog::Table, VehicleAuditLog::ChangedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_vehicle_audit_log_vehicle_id_changed_at")
                    .table(VehicleAuditLog::Table)
                    .col(VehicleAuditLog::VehicleId)
                    .col(VehicleAuditLog::ChangedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(VehicleAuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum VehicleAuditLog {
    Table,
    Id,
    VehicleId,
    ChangedBy,
    ChangeType,
    Snapshot,
    ChangedAt,
}

#[derive(DeriveIden)]
enum Vehicles {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
