use sea_orm_migration::prelude::*;

/// SQLite cannot ALTER a CHECK constraint.  We recreate the `results` table
/// with the expanded `unit_enum` value set while preserving all existing rows.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Create replacement table with updated CHECK constraint.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS results_new (
                id              INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
                event_id        INTEGER  NOT NULL REFERENCES events(id) ON DELETE CASCADE,
                participant_id  INTEGER  NOT NULL REFERENCES users(id),
                attempt_no      INTEGER  NOT NULL,
                value_numeric   REAL     NOT NULL,
                unit_enum       TEXT(32) NOT NULL CHECK (unit_enum IN (
                                    'milliseconds','feet','inches',
                                    'seconds','meters','kilometers','kilograms','points'
                                )),
                entered_by      INTEGER  NOT NULL REFERENCES users(id),
                reviewed_state  TEXT(16) NOT NULL DEFAULT 'pending'
                                    CHECK (reviewed_state IN ('pending','approved','rejected')),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL
            )",
        )
        .await?;

        // 2. Copy all existing rows — column order matches exactly.
        db.execute_unprepared(
            "INSERT INTO results_new
                (id, event_id, participant_id, attempt_no, value_numeric,
                 unit_enum, entered_by, reviewed_state, created_at, updated_at)
             SELECT
                id, event_id, participant_id, attempt_no, value_numeric,
                unit_enum, entered_by, reviewed_state, created_at, updated_at
             FROM results",
        )
        .await?;

        // 3. Drop the old table (CASCADE drops its FKs and constraints).
        db.execute_unprepared("DROP TABLE results").await?;

        // 4. Rename replacement table into place.
        db.execute_unprepared("ALTER TABLE results_new RENAME TO results")
            .await?;

        // 5. Recreate indexes that were dropped with the old table.
        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_results_event_participant_attempt
             ON results (event_id, participant_id, attempt_no)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_results_reviewed_state
             ON results (reviewed_state)",
        )
        .await
        .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // Recreate the original table with the restricted CHECK constraint.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS results_old (
                id              INTEGER  NOT NULL PRIMARY KEY AUTOINCREMENT,
                event_id        INTEGER  NOT NULL REFERENCES events(id) ON DELETE CASCADE,
                participant_id  INTEGER  NOT NULL REFERENCES users(id),
                attempt_no      INTEGER  NOT NULL,
                value_numeric   REAL     NOT NULL,
                unit_enum       TEXT(32) NOT NULL CHECK (unit_enum IN (
                                    'seconds','meters','kilometers','kilograms','points'
                                )),
                entered_by      INTEGER  NOT NULL REFERENCES users(id),
                reviewed_state  TEXT(16) NOT NULL DEFAULT 'pending'
                                    CHECK (reviewed_state IN ('pending','approved','rejected')),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL
            )",
        )
        .await?;

        // Only rows whose unit_enum is still valid in the old constraint survive.
        db.execute_unprepared(
            "INSERT INTO results_old
                (id, event_id, participant_id, attempt_no, value_numeric,
                 unit_enum, entered_by, reviewed_state, created_at, updated_at)
             SELECT
                id, event_id, participant_id, attempt_no, value_numeric,
                unit_enum, entered_by, reviewed_state, created_at, updated_at
             FROM results
             WHERE unit_enum IN ('seconds','meters','kilometers','kilograms','points')",
        )
        .await?;

        db.execute_unprepared("DROP TABLE results").await?;
        db.execute_unprepared("ALTER TABLE results_old RENAME TO results")
            .await?;

        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_results_event_participant_attempt
             ON results (event_id, participant_id, attempt_no)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_results_reviewed_state
             ON results (reviewed_state)",
        )
        .await
        .map(|_| ())
    }
}
