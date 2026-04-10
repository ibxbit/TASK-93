use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UNIT_CHECK: &str =
    "corrected_unit IN ('milliseconds','feet','inches','seconds','meters','kilometers','kilograms','points')";

const STATUS_CHECK: &str = "status IN ('pending', 'approved', 'rejected')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ResultCorrections::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ResultCorrections::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::ResultId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::CorrectedValue)
                            .double()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::CorrectedUnit)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::RequestedBy)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(ResultCorrections::Reason).text().null())
                    .col(
                        ColumnDef::new(ResultCorrections::Status)
                            .string_len(16)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::ResolvedBy)
                            .big_integer()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::ResolvedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(ResultCorrections::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(UNIT_CHECK))
                    .check(Expr::cust(STATUS_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_corrections_result_id")
                            .from(ResultCorrections::Table, ResultCorrections::ResultId)
                            .to(Results::Table, Results::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_corrections_requested_by")
                            .from(ResultCorrections::Table, ResultCorrections::RequestedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_corrections_resolved_by")
                            .from(ResultCorrections::Table, ResultCorrections::ResolvedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Fast lookup of all corrections (including pending) for a given result.
        manager
            .create_index(
                Index::create()
                    .name("idx_result_corrections_result_id")
                    .table(ResultCorrections::Table)
                    .col(ResultCorrections::ResultId)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Enforce single pending correction per result at the DB level.
        // SQLite supports partial indexes.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX IF NOT EXISTS
                 idx_result_corrections_one_pending
                 ON result_corrections (result_id)
                 WHERE status = 'pending'",
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResultCorrections::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ResultCorrections {
    Table,
    Id,
    ResultId,
    CorrectedValue,
    CorrectedUnit,
    RequestedBy,
    Reason,
    Status,
    ResolvedBy,
    ResolvedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Results {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
