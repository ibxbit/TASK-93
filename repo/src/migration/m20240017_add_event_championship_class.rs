use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite supports ADD COLUMN when the column has a DEFAULT value.
        manager
            .alter_table(
                Table::alter()
                    .table(Events::Table)
                    .add_column(
                        ColumnDef::new(Events::IsChampionshipClass)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite cannot DROP COLUMN before 3.35.0; recreate the table without
        // the column to keep the down migration portable.
        let db = manager.get_connection();

        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS events_old (
                id                   INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                name                 TEXT    NOT NULL,
                description          TEXT,
                schedule_group       TEXT,
                venue_identifier     TEXT,
                status               TEXT(16) NOT NULL DEFAULT 'draft'
                                         CHECK (status IN ('draft','published','in_progress','completed','cancelled')),
                published_version_id INTEGER REFERENCES ruleset_versions(id),
                created_by           INTEGER NOT NULL REFERENCES users(id),
                created_at           TIMESTAMPTZ NOT NULL,
                updated_at           TIMESTAMPTZ NOT NULL
            )",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO events_old
                (id, name, description, schedule_group, venue_identifier,
                 status, published_version_id, created_by, created_at, updated_at)
             SELECT
                id, name, description, schedule_group, venue_identifier,
                status, published_version_id, created_by, created_at, updated_at
             FROM events",
        )
        .await?;

        db.execute_unprepared("DROP TABLE events").await?;
        db.execute_unprepared("ALTER TABLE events_old RENAME TO events")
            .await
    }
}

#[derive(DeriveIden)]
enum Events {
    Table,
    IsChampionshipClass,
}
