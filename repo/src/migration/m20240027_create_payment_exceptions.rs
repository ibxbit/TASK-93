use sea_orm_migration::prelude::*;

/// Create `payment_exceptions` — records voids, reversals, and disputes
/// raised against a specific payment entry.
#[derive(DeriveMigrationName)]
pub struct Migration;

const EXCEPTION_TYPE_CHECK: &str = "exception_type IN ('void','reversal','dispute')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS payment_exceptions (
                id             INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                payment_id     INTEGER     NOT NULL
                                   REFERENCES payment_entries(id) ON DELETE RESTRICT,
                exception_type TEXT(16)    NOT NULL CHECK ({EXCEPTION_TYPE_CHECK}),
                reason         TEXT        NOT NULL,
                raised_by      INTEGER     NOT NULL REFERENCES users(id),
                created_at     TIMESTAMPTZ NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_payment_exceptions_payment_id
             ON payment_exceptions (payment_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP TABLE IF EXISTS payment_exceptions")
            .await?;

        Ok(())
    }
}
