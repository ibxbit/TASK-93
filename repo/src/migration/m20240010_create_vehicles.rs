use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const STATUS_CHECK: &str = "status IN ('active', 'inactive', 'retired', 'under_maintenance')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Vehicles::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Vehicles::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // Optional link to the assets register; NULL if vehicle is
                    // tracked operationally but not capitalised.
                    .col(ColumnDef::new(Vehicles::AssetId).big_integer().null())
                    // 17-character Vehicle Identification Number (ISO 3779).
                    .col(
                        ColumnDef::new(Vehicles::Vin)
                            .string_len(17)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Vehicles::RegistrationId).string().not_null())
                    .col(
                        ColumnDef::new(Vehicles::Status)
                            .string_len(24)
                            .not_null()
                            .default("active"),
                    )
                    .col(
                        ColumnDef::new(Vehicles::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Vehicles::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(STATUS_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_vehicles_asset_id")
                            .from(Vehicles::Table, Vehicles::AssetId)
                            .to(Assets::Table, Assets::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_vehicles_status")
                    .table(Vehicles::Table)
                    .col(Vehicles::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Vehicles::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Vehicles {
    Table,
    Id,
    AssetId,
    Vin,
    RegistrationId,
    Status,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Assets {
    Table,
    Id,
}
