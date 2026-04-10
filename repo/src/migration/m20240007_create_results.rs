use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

const UNIT_CHECK: &str = "unit_enum IN ('seconds', 'meters', 'kilometers', 'kilograms', 'points')";

const REVIEWED_STATE_CHECK: &str = "reviewed_state IN ('pending', 'approved', 'rejected')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Results::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Results::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Results::EventId).big_integer().not_null())
                    // Participant references users for now; will point to a
                    // dedicated participants table when that module is added.
                    .col(
                        ColumnDef::new(Results::ParticipantId)
                            .big_integer()
                            .not_null(),
                    )
                    // 1-based attempt counter within the event for this participant.
                    .col(ColumnDef::new(Results::AttemptNo).integer().not_null())
                    // Measurement value (race time, distance, etc.).
                    .col(ColumnDef::new(Results::ValueNumeric).double().not_null())
                    .col(ColumnDef::new(Results::UnitEnum).string_len(32).not_null())
                    .col(ColumnDef::new(Results::EnteredBy).big_integer().not_null())
                    .col(
                        ColumnDef::new(Results::ReviewedState)
                            .string_len(16)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Results::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Results::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .check(Expr::cust(UNIT_CHECK))
                    .check(Expr::cust(REVIEWED_STATE_CHECK))
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_results_event_id")
                            .from(Results::Table, Results::EventId)
                            .to(Events::Table, Events::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_results_participant_id")
                            .from(Results::Table, Results::ParticipantId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_results_entered_by")
                            .from(Results::Table, Results::EnteredBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // One attempt row per participant per event.
        manager
            .create_index(
                Index::create()
                    .name("idx_results_event_participant_attempt")
                    .table(Results::Table)
                    .col(Results::EventId)
                    .col(Results::ParticipantId)
                    .col(Results::AttemptNo)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        // Review queue filter.
        manager
            .create_index(
                Index::create()
                    .name("idx_results_reviewed_state")
                    .table(Results::Table)
                    .col(Results::ReviewedState)
                    .if_not_exists()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Results::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Results {
    Table,
    Id,
    EventId,
    ParticipantId,
    AttemptNo,
    ValueNumeric,
    UnitEnum,
    EnteredBy,
    ReviewedState,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Events {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}
