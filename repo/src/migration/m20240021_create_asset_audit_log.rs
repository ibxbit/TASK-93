use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CHANGE_TYPE_CHECK: &str =
    "change_type IN ('created', 'updated', 'status_changed', 'owner_changed', 'imported')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(AssetAuditLog::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(AssetAuditLog::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(AssetAuditLog::AssetId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AssetAuditLog::ChangedBy)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(AssetAuditLog::ChangeType)
                            .string_len(24)
                            .not_null(),
                    )
                    // Full JSON snapshot of the asset after the change.
                    .col(ColumnDef::new(AssetAuditLog::Snapshot).text().not_null())
                    .col(
                        ColumnDef::new(AssetAuditLog::ChangedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(CHANGE_TYPE_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_asset_audit_log_asset_id")
                            .from(AssetAuditLog::Table, AssetAuditLog::AssetId)
                            .to(Assets::Table, Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_asset_audit_log_changed_by")
                            .from(AssetAuditLog::Table, AssetAuditLog::ChangedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Chronological history lookup per asset.
        manager
            .create_index(
                Index::create()
                    .name("idx_asset_audit_log_asset_id_changed_at")
                    .table(AssetAuditLog::Table)
                    .col(AssetAuditLog::AssetId)
                    .col(AssetAuditLog::ChangedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(AssetAuditLog::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum AssetAuditLog {
    Table,
    Id,
    AssetId,
    ChangedBy,
    ChangeType,
    Snapshot,
    ChangedAt,
}

#[derive(DeriveIden)]
enum Assets {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
