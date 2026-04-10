use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const PRICING_MODEL_CHECK: &str = "pricing_model IN ('fixed', 'per_unit', 'percentage')";

const ADJUSTMENT_TYPE_CHECK: &str =
    "adjustment_type IS NULL OR adjustment_type IN ('discount', 'surcharge')";

const AMOUNTS_CHECK: &str = "quantity > 0 AND unit_price >= 0 AND line_total >= 0";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(InvoiceLines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(InvoiceLines::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(InvoiceLines::InvoiceId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InvoiceLines::Description)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InvoiceLines::PricingModel)
                            .string_len(16)
                            .not_null(),
                    )
                    // Stored as REAL; quantities like "2.5 hours" need fractional precision.
                    .col(ColumnDef::new(InvoiceLines::Quantity).double().not_null())
                    .col(
                        ColumnDef::new(InvoiceLines::UnitPrice)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    // NULL means no adjustment on this line.
                    .col(
                        ColumnDef::new(InvoiceLines::AdjustmentType)
                            .string_len(16)
                            .null(),
                    )
                    // Absolute amount (discount) or basis points (percentage surcharge).
                    .col(
                        ColumnDef::new(InvoiceLines::AdjustmentValue)
                            .decimal_len(19, 4)
                            .null(),
                    )
                    .col(
                        ColumnDef::new(InvoiceLines::LineTotal)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(InvoiceLines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(PRICING_MODEL_CHECK))
                    .check(Expr::cust(ADJUSTMENT_TYPE_CHECK))
                    .check(Expr::cust(AMOUNTS_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_invoice_lines_invoice_id")
                            .from(InvoiceLines::Table, InvoiceLines::InvoiceId)
                            .to(Invoices::Table, Invoices::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_invoice_lines_invoice_id")
                    .table(InvoiceLines::Table)
                    .col(InvoiceLines::InvoiceId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(InvoiceLines::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum InvoiceLines {
    Table,
    Id,
    InvoiceId,
    Description,
    PricingModel,
    Quantity,
    UnitPrice,
    AdjustmentType,
    AdjustmentValue,
    LineTotal,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Invoices {
    Table,
    Id,
}
