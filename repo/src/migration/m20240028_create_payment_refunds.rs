use sea_orm_migration::prelude::*;

/// Create `payment_refunds` — two-stage approval workflow (finance → auditor)
/// for refunds against a specific payment entry.
///
/// `invoice_line_id` is populated on approval and links the credit memo line.
#[derive(DeriveMigrationName)]
pub struct Migration;

const AMOUNT_CHECK: &str = "amount > 0";

const STATUS_CHECK: &str = "status IN ('pending_finance','pending_auditor','approved','rejected')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS payment_refunds (
                id                   INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                payment_id           INTEGER       NOT NULL
                                         REFERENCES payment_entries(id) ON DELETE RESTRICT,
                invoice_line_id      INTEGER
                                         REFERENCES invoice_lines(id),
                amount               DECIMAL(16,4) NOT NULL CHECK ({AMOUNT_CHECK}),
                reason               TEXT          NOT NULL,
                status               TEXT(24)      NOT NULL DEFAULT 'pending_finance'
                                         CHECK ({STATUS_CHECK}),
                requested_by         INTEGER       NOT NULL REFERENCES users(id),
                finance_approved_by  INTEGER       REFERENCES users(id),
                auditor_approved_by  INTEGER       REFERENCES users(id),
                rejected_by          INTEGER       REFERENCES users(id),
                rejection_reason     TEXT,
                created_at           TIMESTAMPTZ   NOT NULL,
                updated_at           TIMESTAMPTZ   NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_payment_refunds_payment_id
             ON payment_refunds (payment_id)",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("DROP TABLE IF EXISTS payment_refunds")
            .await?;

        Ok(())
    }
}
