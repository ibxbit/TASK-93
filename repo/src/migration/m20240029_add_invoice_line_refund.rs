use sea_orm_migration::prelude::*;

/// Recreate `invoice_lines` to:
///   1. Add `is_refund INTEGER NOT NULL DEFAULT 0`.
///   2. Relax AMOUNTS_CHECK to allow `line_total < 0` when `is_refund = 1`.
///
/// Uses the SQLite FK-off / create-new / INSERT-SELECT / DROP / RENAME pattern.
#[derive(DeriveMigrationName)]
pub struct Migration;

const PRICING_MODEL_CHECK: &str =
    "pricing_model IN ('fixed','per_unit','percentage','per_duration','package')";

const ADJUSTMENT_TYPE_CHECK: &str =
    "adjustment_type IS NULL OR adjustment_type IN ('discount','surcharge')";

const AMOUNTS_CHECK: &str =
    "quantity > 0 AND unit_price >= 0 AND (line_total >= 0 OR is_refund = 1)";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoice_lines_new (
                id                       INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id               INTEGER       NOT NULL
                                             REFERENCES invoices(id) ON DELETE CASCADE,
                description              TEXT          NOT NULL,
                pricing_model            TEXT(16)      NOT NULL CHECK ({PRICING_MODEL_CHECK}),
                quantity                 REAL          NOT NULL,
                unit_price               DECIMAL(16,4) NOT NULL,
                adjustment_type          TEXT(16)      CHECK ({ADJUSTMENT_TYPE_CHECK}),
                adjustment_is_percentage INTEGER       NOT NULL DEFAULT 0,
                adjustment_value         DECIMAL(16,4),
                line_total               DECIMAL(16,4) NOT NULL,
                is_refund                INTEGER       NOT NULL DEFAULT 0,
                created_at               TIMESTAMPTZ   NOT NULL,
                CHECK ({AMOUNTS_CHECK})
            )"
        ))
        .await?;

        // Carry forward all existing rows; default is_refund = 0.
        db.execute_unprepared(
            "INSERT INTO invoice_lines_new
                (id, invoice_id, description, pricing_model, quantity,
                 unit_price, adjustment_type, adjustment_is_percentage,
                 adjustment_value, line_total, is_refund, created_at)
             SELECT
                id, invoice_id, description, pricing_model, quantity,
                unit_price, adjustment_type, adjustment_is_percentage,
                adjustment_value, line_total, 0, created_at
             FROM invoice_lines",
        )
        .await?;

        db.execute_unprepared("DROP TABLE invoice_lines").await?;
        db.execute_unprepared("ALTER TABLE invoice_lines_new RENAME TO invoice_lines")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice_id
             ON invoice_lines (invoice_id)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // Recreate without is_refund and with the original AMOUNTS_CHECK.
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoice_lines_old (
                id                       INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id               INTEGER       NOT NULL
                                             REFERENCES invoices(id) ON DELETE CASCADE,
                description              TEXT          NOT NULL,
                pricing_model            TEXT(16)      NOT NULL CHECK ({PRICING_MODEL_CHECK}),
                quantity                 REAL          NOT NULL,
                unit_price               DECIMAL(16,4) NOT NULL,
                adjustment_type          TEXT(16)      CHECK ({ADJUSTMENT_TYPE_CHECK}),
                adjustment_is_percentage INTEGER       NOT NULL DEFAULT 0,
                adjustment_value         DECIMAL(16,4),
                line_total               DECIMAL(16,4) NOT NULL,
                created_at               TIMESTAMPTZ   NOT NULL,
                CHECK (quantity > 0 AND unit_price >= 0 AND line_total >= 0)
            )"
        ))
        .await?;

        // Only non-refund rows survive; refund rows are dropped.
        db.execute_unprepared(
            "INSERT INTO invoice_lines_old
                (id, invoice_id, description, pricing_model, quantity,
                 unit_price, adjustment_type, adjustment_is_percentage,
                 adjustment_value, line_total, created_at)
             SELECT
                id, invoice_id, description, pricing_model, quantity,
                unit_price, adjustment_type, adjustment_is_percentage,
                adjustment_value, line_total, created_at
             FROM invoice_lines
             WHERE is_refund = 0",
        )
        .await?;

        db.execute_unprepared("DROP TABLE invoice_lines").await?;
        db.execute_unprepared("ALTER TABLE invoice_lines_old RENAME TO invoice_lines")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice_id
             ON invoice_lines (invoice_id)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
