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
                    .table(ResultReviews::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ResultReviews::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ResultReviews::ResultId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultReviews::RefereeId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultReviews::Decision)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ResultReviews::Comment).text().null())
                    .col(
                        ColumnDef::new(ResultReviews::ReviewedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ResultReviews::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(DECISION_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_reviews_result_id")
                            .from(ResultReviews::Table, ResultReviews::ResultId)
                            .to(Results::Table, Results::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_result_reviews_referee_id")
                            .from(ResultReviews::Table, ResultReviews::RefereeId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // One decision per referee per result — prevents double-submission.
        manager
            .create_index(
                Index::create()
                    .name("idx_result_reviews_result_referee")
                    .table(ResultReviews::Table)
                    .col(ResultReviews::ResultId)
                    .col(ResultReviews::RefereeId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ResultReviews::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum ResultReviews {
    Table,
    Id,
    ResultId,
    RefereeId,
    Decision,
    Comment,
    ReviewedAt,
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
