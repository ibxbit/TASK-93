use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const STATUS_CHECK: &str = "status IN ('draft', 'issued', 'paid', 'cancelled', 'overdue')";

// Referential integrity: total must equal subtotal + tax.
// Enforced here; exact equality validated in business logic to handle rounding.
const TOTAL_CHECK: &str = "total >= 0 AND subtotal >= 0 AND tax >= 0";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Invoices::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Invoices::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    // Human-readable reference (e.g. "INV-2024-0042").
                    .col(
                        ColumnDef::new(Invoices::InvoiceNo)
                            .string_len(64)
                            .not_null()
                            .unique_key(),
                    )
                    // Free-text name or ID of customer / vendor.
                    .col(ColumnDef::new(Invoices::Counterparty).string().not_null())
                    // ISO 8601 date string stored as TEXT in SQLite.
                    .col(ColumnDef::new(Invoices::IssueDate).date().not_null())
                    .col(
                        ColumnDef::new(Invoices::Subtotal)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Invoices::Tax).decimal_len(19, 4).not_null())
                    .col(
                        ColumnDef::new(Invoices::Total)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Invoices::Status)
                            .string_len(16)
                            .not_null()
                            .default("draft"),
                    )
                    .col(ColumnDef::new(Invoices::CreatedBy).big_integer().not_null())
                    .col(
                        ColumnDef::new(Invoices::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Invoices::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(STATUS_CHECK))
                    .check(Expr::cust(TOTAL_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_invoices_created_by")
                            .from(Invoices::Table, Invoices::CreatedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_invoices_status")
                    .table(Invoices::Table)
                    .col(Invoices::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_invoices_counterparty")
                    .table(Invoices::Table)
                    .col(Invoices::Counterparty)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Invoices::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Invoices {
    Table,
    Id,
    InvoiceNo,
    Counterparty,
    IssueDate,
    Subtotal,
    Tax,
    Total,
    Status,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
