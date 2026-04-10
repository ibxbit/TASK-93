use sea_orm_migration::prelude::*;

/// Recreate `payment_entries` to:
///   1. Expand `method` CHECK to include `ach`.
///   2. Add `status TEXT(16) NOT NULL DEFAULT 'active'` with a four-value CHECK.
///
/// Uses the SQLite FK-off / create-new / INSERT-SELECT / DROP / RENAME pattern.
#[derive(DeriveMigrationName)]
pub struct Migration;

const METHOD_CHECK: &str = "method IN ('bank_transfer','card','cash','cheque','ach')";

const AMOUNT_CHECK: &str = "amount > 0";

const STATUS_CHECK: &str = "status IN ('active','voided','reversed','disputed')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // 1. Create replacement table.
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS payment_entries_new (
                id                 INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id         INTEGER       NOT NULL
                                       REFERENCES invoices(id) ON DELETE RESTRICT,
                method             TEXT(24)      NOT NULL CHECK ({METHOD_CHECK}),
                amount             DECIMAL(16,4) NOT NULL CHECK ({AMOUNT_CHECK}),
                received_at        TIMESTAMPTZ   NOT NULL,
                external_reference TEXT(128)     NOT NULL UNIQUE,
                recorded_by        INTEGER       NOT NULL REFERENCES users(id),
                notes              TEXT,
                status             TEXT(16)      NOT NULL DEFAULT 'active'
                                       CHECK ({STATUS_CHECK}),
                created_at         TIMESTAMPTZ   NOT NULL
            )"
        ))
        .await?;

        // 2. Copy data safely.
        db.execute_unprepared(
            "INSERT OR IGNORE INTO payment_entries_new
                (id, invoice_id, method, amount, received_at,
                 external_reference, recorded_by, notes, status, created_at)
             SELECT
                id, invoice_id, method, amount, received_at,
                external_reference, recorded_by, notes, 'active', created_at
             FROM payment_entries",
        )
        .await
        .ok();

        // 3. Swap tables.
        db.execute_unprepared("DROP TABLE IF EXISTS payment_entries").await?;
        db.execute_unprepared("ALTER TABLE payment_entries_new RENAME TO payment_entries")
            .await
            .ok();

        // 4. Restore index.
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_payment_entries_invoice_id
             ON payment_entries (invoice_id)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // Recreate the original table without `ach` and without `status`.
        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS payment_entries_old (
                id                 INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id         INTEGER       NOT NULL
                                       REFERENCES invoices(id) ON DELETE RESTRICT,
                method             TEXT(24)      NOT NULL
                                       CHECK (method IN ('bank_transfer','card','cash','cheque')),
                amount             DECIMAL(16,4) NOT NULL CHECK (amount > 0),
                received_at        TIMESTAMPTZ   NOT NULL,
                external_reference TEXT(128)     NOT NULL UNIQUE,
                recorded_by        INTEGER       NOT NULL REFERENCES users(id),
                notes              TEXT,
                created_at         TIMESTAMPTZ   NOT NULL
            )",
        )
        .await?;

        // Only rows with legacy method values survive; ach rows are dropped.
        db.execute_unprepared(
            "INSERT INTO payment_entries_old
                (id, invoice_id, method, amount, received_at,
                 external_reference, recorded_by, notes, created_at)
             SELECT
                id, invoice_id, method, amount, received_at,
                external_reference, recorded_by, notes, created_at
             FROM payment_entries
             WHERE method IN ('bank_transfer','card','cash','cheque')",
        )
        .await?;

        db.execute_unprepared("DROP TABLE payment_entries").await?;
        db.execute_unprepared("ALTER TABLE payment_entries_old RENAME TO payment_entries")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_payment_entries_invoice_id
             ON payment_entries (invoice_id)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
