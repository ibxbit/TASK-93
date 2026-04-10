use sea_orm_migration::prelude::*;

pub struct Migration;

impl MigrationName for Migration {
    fn name(&self) -> &str {
        "m20240035_create_audit_log"
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Unified append-only audit trail for all critical operations.
        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS audit_log (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                actor_id    INTEGER NOT NULL REFERENCES users(id),
                action      TEXT    NOT NULL,
                entity_type TEXT    NOT NULL,
                entity_id   INTEGER NOT NULL,
                snapshot    TEXT    NOT NULL DEFAULT '{}',
                metadata    TEXT    NOT NULL DEFAULT '{}',
                created_at  TEXT    NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
            )",
        )
        .await?;

        // ── Indexes for queryability ───────────────────────────────────────────

        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_actor_id
             ON audit_log(actor_id)",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_entity
             ON audit_log(entity_type, entity_id)",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_action
             ON audit_log(action)",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_audit_log_created_at
             ON audit_log(created_at DESC)",
        )
        .await?;

        // ── Immutability triggers ─────────────────────────────────────────────
        // The audit log is permanently append-only.  Any UPDATE or DELETE
        // attempt is rejected at the database engine level — no application-layer
        // bypass can circumvent these triggers.

        conn.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS audit_log_no_update
             BEFORE UPDATE ON audit_log
             BEGIN
                 SELECT RAISE(FAIL, 'audit_log is immutable: updates are not permitted');
             END",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE TRIGGER IF NOT EXISTS audit_log_no_delete
             BEFORE DELETE ON audit_log
             BEGIN
                 SELECT RAISE(FAIL, 'audit_log is immutable: deletions are not permitted');
             END",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();
        conn.execute_unprepared("DROP TRIGGER IF EXISTS audit_log_no_delete")
            .await?;
        conn.execute_unprepared("DROP TRIGGER IF EXISTS audit_log_no_update")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS audit_log")
            .await?;
        Ok(())
    }
}
