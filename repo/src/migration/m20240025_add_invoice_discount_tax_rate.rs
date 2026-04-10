use sea_orm_migration::prelude::*;

/// Add billing fields to `invoices`:
///   - tax_rate       DECIMAL(5,4) NOT NULL DEFAULT 0
///   - discount_type  TEXT         (nullable)
///   - discount_value DECIMAL(19,4)(nullable)
///   - discount_amount DECIMAL(19,4) NOT NULL DEFAULT 0
///
/// SQLite supports ADD COLUMN with a constant DEFAULT, so four separate
/// ALTER TABLE statements are used instead of a full table recreation.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(
            "ALTER TABLE invoices ADD COLUMN tax_rate DECIMAL(5,4) NOT NULL DEFAULT 0",
        )
        .await?;

        // Nullable — no CHECK constraint; validated at service layer ("percentage"|"fixed_amount").
        db.execute_unprepared("ALTER TABLE invoices ADD COLUMN discount_type TEXT")
            .await?;

        db.execute_unprepared("ALTER TABLE invoices ADD COLUMN discount_value DECIMAL(19,4)")
            .await?;

        db.execute_unprepared(
            "ALTER TABLE invoices ADD COLUMN discount_amount DECIMAL(19,4) NOT NULL DEFAULT 0",
        )
        .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite ≥ 3.35 supports DROP COLUMN, but for maximum portability we
        // recreate the table without the new columns.
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS invoices_old (
                id           INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_no   TEXT(64)      NOT NULL UNIQUE,
                counterparty TEXT          NOT NULL,
                issue_date   DATE          NOT NULL,
                subtotal     DECIMAL(19,4) NOT NULL,
                tax          DECIMAL(19,4) NOT NULL,
                total        DECIMAL(19,4) NOT NULL,
                status       TEXT(16)      NOT NULL DEFAULT 'draft'
                                 CHECK (status IN ('draft','issued','paid','cancelled','overdue')),
                created_by   INTEGER       NOT NULL REFERENCES users(id),
                created_at   TIMESTAMPTZ   NOT NULL,
                updated_at   TIMESTAMPTZ   NOT NULL,
                CHECK (total >= 0 AND subtotal >= 0 AND tax >= 0)
            )",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO invoices_old
                (id, invoice_no, counterparty, issue_date,
                 subtotal, tax, total, status, created_by, created_at, updated_at)
             SELECT
                id, invoice_no, counterparty, issue_date,
                subtotal, tax, total, status, created_by, created_at, updated_at
             FROM invoices",
        )
        .await?;

        db.execute_unprepared("DROP TABLE invoices").await?;
        db.execute_unprepared("ALTER TABLE invoices_old RENAME TO invoices")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices (status)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoices_counterparty ON invoices (counterparty)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
