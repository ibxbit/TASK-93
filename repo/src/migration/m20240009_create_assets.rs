use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CATEGORY_CHECK: &str =
    "category IN ('vehicle', 'equipment', 'facility', 'electronic', 'other')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Assets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Assets::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // Human-readable unique identifier (e.g. "ASSET-2024-001").
                    .col(
                        ColumnDef::new(Assets::AssetCode)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Assets::Category).string_len(32).not_null())
                    .col(ColumnDef::new(Assets::Brand).string().not_null())
                    .col(ColumnDef::new(Assets::Model).string().not_null())
                    .col(ColumnDef::new(Assets::SerialNumber).string().null())
                    // Straight-line depreciation period in months.
                    .col(
                        ColumnDef::new(Assets::DepreciationMonths)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(Assets::Notes).text().null())
                    .col(
                        ColumnDef::new(Assets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Assets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(CATEGORY_CHECK))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_assets_category")
                    .table(Assets::Table)
                    .col(Assets::Category)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Assets::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Assets {
    Table,
    Id,
    AssetCode,
    Category,
    Brand,
    Model,
    SerialNumber,
    DepreciationMonths,
    Notes,
    CreatedAt,
    UpdatedAt,
}
