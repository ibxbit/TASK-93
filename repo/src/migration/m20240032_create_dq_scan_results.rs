use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS dq_scan_results (
                id               INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                entity           TEXT    NOT NULL,
                checks_run       TEXT    NOT NULL,
                zscore_threshold REAL    NOT NULL DEFAULT 3.0,
                numeric_fields   TEXT    NOT NULL DEFAULT '[]',
                hash_fields      TEXT    NOT NULL DEFAULT '[]',
                total_records    INTEGER NOT NULL DEFAULT 0,
                anomaly_count    INTEGER NOT NULL DEFAULT 0,
                report           TEXT    NOT NULL DEFAULT '[]',
                created_by       INTEGER NOT NULL REFERENCES users(id),
                created_at       TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_dq_scan_results_entity \
             ON dq_scan_results(entity)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_dq_scan_results_created_at \
             ON dq_scan_results(created_at DESC)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS dq_scan_results")
            .await?;
        Ok(())
    }
}
