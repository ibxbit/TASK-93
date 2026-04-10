use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

// Valid values for the status column.
const STATUS_CHECK: &str =
    "status IN ('draft', 'published', 'in_progress', 'completed', 'cancelled')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Events::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Events::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Events::Name).string().not_null())
                    .col(ColumnDef::new(Events::Description).text().null())
                    // Logical grouping (e.g. "2024 Championship Round 1–5").
                    .col(ColumnDef::new(Events::ScheduleGroup).string().null())
                    // TEXT column constrained to known statuses via CHECK.
                    .col(
                        ColumnDef::new(Events::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    // FK to the ruleset_version active when the event was published.
                    .col(
                        ColumnDef::new(Events::PublishedVersionId)
                            .big_integer()
                            .null(),
                    )
                    .col(ColumnDef::new(Events::CreatedBy).big_integer().not_null())
                    .col(
                        ColumnDef::new(Events::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Events::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(STATUS_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_events_published_version_id")
                            .from(Events::Table, Events::PublishedVersionId)
                            .to(RulesetVersions::Table, RulesetVersions::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_events_created_by")
                            .from(Events::Table, Events::CreatedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Efficient filtering by schedule group and status.
        manager
            .create_index(
                Index::create()
                    .name("idx_events_schedule_group")
                    .table(Events::Table)
                    .col(Events::ScheduleGroup)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_events_status")
                    .table(Events::Table)
                    .col(Events::Status)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Events::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Events {
    Table,
    Id,
    Name,
    Description,
    ScheduleGroup,
    Status,
    PublishedVersionId,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum RulesetVersions {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
