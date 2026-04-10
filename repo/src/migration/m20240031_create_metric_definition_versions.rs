use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS metric_definition_versions (
                id            INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                metric_id     INTEGER NOT NULL REFERENCES metric_definitions(id) ON DELETE CASCADE,
                version       INTEGER NOT NULL,
                definition    TEXT NOT NULL,
                changed_by    INTEGER REFERENCES users(id),
                change_reason TEXT,
                created_at    TIMESTAMPTZ NOT NULL,
                UNIQUE (metric_id, version)
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_metric_def_versions_metric_id \
             ON metric_definition_versions(metric_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS metric_definition_versions")
            .await?;

        Ok(())
    }
}
