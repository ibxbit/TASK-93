use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const DECISION_CHECK: &str = "decision IN ('approved', 'rejected')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ResultArbitrations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ResultArbitrations::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ResultArbitrations::ResultId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultArbitrations::ArbitratedBy)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultArbitrations::Decision)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ResultArbitrations::Comment).text().null())
                    .col(
                        ColumnDef::new(ResultArbitrations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(DECISION_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_arbitrations_result_id")
                            .from(ResultArbitrations::Table, ResultArbitrations::ResultId)
                            .to(Results::Table, Results::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_arbitrations_arbitrated_by")
                            .from(ResultArbitrations::Table, ResultArbitrations::ArbitratedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // One binding arbitration per result — cannot be re-arbitrated once recorded.
        manager
            .create_index(
                Index::create()
                    .name("idx_result_arbitrations_result_id")
                    .table(ResultArbitrations::Table)
                    .col(ResultArbitrations::ResultId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResultArbitrations::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ResultArbitrations {
    Table,
    Id,
    ResultId,
    ArbitratedBy,
    Decision,
    Comment,
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
