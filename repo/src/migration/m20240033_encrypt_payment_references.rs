use sea_orm_migration::prelude::*;

/// Adds a `reference_hash` blind-index column to `payment_entries` to support
/// equality lookups on the now-encrypted `external_reference` field.
///
/// The unique constraint moves from `external_reference` (plaintext) to
/// `reference_hash` (keyed digest).  Existing rows have their hash initialised
/// to the plaintext reference value — a one-time re-encryption job should be
/// run against any pre-existing data in a production deployment.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS payment_entries_new (
                id                 INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id         INTEGER NOT NULL REFERENCES invoices(id) ON DELETE RESTRICT,
                method             TEXT(16) NOT NULL
                                       CHECK (method IN ('bank_transfer','card','cash','cheque','ach')),
                amount             NUMERIC  NOT NULL CHECK (amount > 0),
                received_at        TIMESTAMPTZ NOT NULL,
                external_reference TEXT NOT NULL,
                reference_hash     TEXT NOT NULL DEFAULT '' UNIQUE,
                recorded_by        INTEGER NOT NULL REFERENCES users(id),
                notes              TEXT,
                status             TEXT(16) NOT NULL DEFAULT 'active'
                                       CHECK (status IN ('active','voided','reversed','disputed')),
                created_at         TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        // Seed reference_hash with the existing plaintext reference so that
        // idempotency lookups still function for any pre-existing rows.
        db.execute_unprepared(
            r#"
            INSERT INTO payment_entries_new
                (id, invoice_id, method, amount, received_at,
                 external_reference, reference_hash,
                 recorded_by, notes, status, created_at)
            SELECT id, invoice_id, method, amount, received_at,
                   external_reference, external_reference,
                   recorded_by, notes, status, created_at
            FROM payment_entries
            "#,
        )
        .await?;

        db.execute_unprepared("DROP TABLE payment_entries").await?;
        db.execute_unprepared("ALTER TABLE payment_entries_new RENAME TO payment_entries")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_payment_entries_invoice_id \
             ON payment_entries(invoice_id)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS payment_entries_old (
                id                 INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id         INTEGER NOT NULL REFERENCES invoices(id) ON DELETE RESTRICT,
                method             TEXT(16) NOT NULL
                                       CHECK (method IN ('bank_transfer','card','cash','cheque','ach')),
                amount             NUMERIC  NOT NULL CHECK (amount > 0),
                received_at        TIMESTAMPTZ NOT NULL,
                external_reference TEXT NOT NULL UNIQUE,
                recorded_by        INTEGER NOT NULL REFERENCES users(id),
                notes              TEXT,
                status             TEXT(16) NOT NULL DEFAULT 'active'
                                       CHECK (status IN ('active','voided','reversed','disputed')),
                created_at         TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            INSERT INTO payment_entries_old
                (id, invoice_id, method, amount, received_at,
                 external_reference, recorded_by, notes, status, created_at)
            SELECT id, invoice_id, method, amount, received_at,
                   external_reference, recorded_by, notes, status, created_at
            FROM payment_entries
            "#,
        )
        .await?;

        db.execute_unprepared("DROP TABLE payment_entries").await?;
        db.execute_unprepared("ALTER TABLE payment_entries_old RENAME TO payment_entries")
            .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
