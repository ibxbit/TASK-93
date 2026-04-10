use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS metric_definitions (
                id         INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                name       TEXT(128) NOT NULL UNIQUE,
                definition TEXT NOT NULL,
                unit       TEXT(32),
                category   TEXT(32) NOT NULL
                               CHECK (category IN ('financial','operational','results','assets')),
                version    INTEGER NOT NULL DEFAULT 1,
                owner_id   INTEGER REFERENCES users(id),
                is_active  INTEGER NOT NULL DEFAULT 1,
                created_at TIMESTAMPTZ NOT NULL,
                updated_at TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_metric_definitions_name \
             ON metric_definitions(name)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_metric_definitions_category \
             ON metric_definitions(category)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS metric_definitions")
            .await?;

        Ok(())
    }
}
