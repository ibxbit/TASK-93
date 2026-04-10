use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const METHOD_CHECK: &str = "method IN ('bank_transfer', 'card', 'cash', 'cheque')";

const AMOUNT_CHECK: &str = "amount > 0";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PaymentEntries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PaymentEntries::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PaymentEntries::InvoiceId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentEntries::Method)
                            .string_len(24)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentEntries::Amount)
                            .decimal_len(19, 4)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentEntries::ReceivedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    // Reference from the external payment system; unique prevents
                    // duplicate processing of the same transaction.
                    .col(
                        ColumnDef::new(PaymentEntries::ExternalReference)
                            .string_len(128)
                            .not_null()
                            .unique_key(),
                    )
                    .col(
                        ColumnDef::new(PaymentEntries::RecordedBy)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PaymentEntries::Notes).text().null())
                    .col(
                        ColumnDef::new(PaymentEntries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(METHOD_CHECK))
                    .check(Expr::cust(AMOUNT_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_payment_entries_invoice_id")
                            .from(PaymentEntries::Table, PaymentEntries::InvoiceId)
                            .to(Invoices::Table, Invoices::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_payment_entries_recorded_by")
                            .from(PaymentEntries::Table, PaymentEntries::RecordedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Fast lookup of all payments for an invoice.
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_entries_invoice_id")
                    .table(PaymentEntries::Table)
                    .col(PaymentEntries::InvoiceId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PaymentEntries::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum PaymentEntries {
    Table,
    Id,
    InvoiceId,
    Method,
    Amount,
    ReceivedAt,
    ExternalReference,
    RecordedBy,
    Notes,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Invoices {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
