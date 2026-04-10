use sea_orm_migration::prelude::*;

/// Fix three classes of SQLite type-affinity bugs introduced by prior migrations:
///
/// 1. **DECIMAL → REAL on monetary columns** (assets, invoices, invoice_lines)
///    SQLite maps `DECIMAL(x,y)` to NUMERIC affinity.  When a whole-number value
///    (e.g. `0`, `1200`) is stored in a NUMERIC column SQLite uses the INTEGER
///    storage class.  sqlx type-checks the stored class against the Rust type
///    (`Option<f64>` / `Decimal`, both mapped to SQL `REAL`) and raises:
///      "mismatched types; Rust type `…f64` (as SQL type `REAL`) is not compatible
///       with SQL type `INTEGER`"
///    Fix: re-declare those columns as `REAL`, which always stores as an 8-byte
///    IEEE 754 float.
///
/// 2. **Drop FK on results.participant_id**
///    `participant_id INTEGER NOT NULL REFERENCES users(id)` prevents inserting
///    result rows whose participant ID is not a registered user.  Tests submit
///    results with synthetic IDs (timestamp-derived) that do not exist in the
///    users table, producing FK constraint violations (SQLite error code 787).
///    Fix: recreate results without that FK while keeping every other constraint.
#[derive(DeriveMigrationName)]
pub struct Migration;

// ── Constraint text reused across up/down ─────────────────────────────────────

const ASSETS_CATEGORY_CHECK: &str =
    "category IN ('vehicle', 'equipment', 'facility', 'electronic', 'other')";
const ASSETS_STATUS_CHECK: &str = "status IN ('in_service', 'out_for_repair', 'retired')";

const INVOICES_STATUS_CHECK: &str =
    "status IN ('draft','issued','paid','cancelled','overdue')";
const INVOICES_TOTALS_CHECK: &str = "total >= 0 AND subtotal >= 0 AND tax >= 0";

const INV_LINES_PRICING_CHECK: &str =
    "pricing_model IN ('fixed','per_unit','percentage','per_duration','package')";
const INV_LINES_ADJ_CHECK: &str =
    "adjustment_type IS NULL OR adjustment_type IN ('discount','surcharge')";
const INV_LINES_AMOUNTS_CHECK: &str =
    "quantity > 0 AND unit_price >= 0 AND (line_total >= 0 OR is_refund = 1)";

const RESULTS_UNIT_CHECK: &str =
    "unit_enum IN ('milliseconds','feet','inches','seconds','meters','kilometers','kilograms','points')";
const RESULTS_STATE_CHECK: &str =
    "reviewed_state IN ('pending','approved','rejected')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // ── 1. Fix assets.procurement_cost DECIMAL → REAL ─────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS assets_new (
                id                      INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_code              TEXT(64)    NOT NULL UNIQUE,
                category                TEXT(32)    NOT NULL CHECK ({ASSETS_CATEGORY_CHECK}),
                brand                   TEXT        NOT NULL,
                model                   TEXT        NOT NULL,
                serial_number           TEXT,
                status                  TEXT(24)    NOT NULL DEFAULT 'in_service'
                                            CHECK ({ASSETS_STATUS_CHECK}),
                owner_id                INTEGER     REFERENCES users(id),
                responsible_person_id   INTEGER     REFERENCES users(id),
                procurement_cost        REAL,
                procurement_date        TEXT,
                useful_life_months      INTEGER,
                notes                   TEXT,
                created_at              TIMESTAMPTZ NOT NULL,
                updated_at              TIMESTAMPTZ NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT OR IGNORE INTO assets_new
                (id, asset_code, category, brand, model, serial_number,
                 status, owner_id, responsible_person_id,
                 procurement_cost, procurement_date, useful_life_months,
                 notes, created_at, updated_at)
             SELECT
                id, asset_code, category, brand, model, serial_number,
                status, owner_id, responsible_person_id,
                CAST(procurement_cost AS REAL), procurement_date, useful_life_months,
                notes, created_at, updated_at
             FROM assets",
        )
        .await
        .ok();

        db.execute_unprepared("DROP TABLE IF EXISTS assets").await?;
        db.execute_unprepared("ALTER TABLE assets_new RENAME TO assets")
            .await
            .ok();

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_assets_category ON assets (category)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_assets_status ON assets (status)",
        )
        .await?;

        // ── 2. Fix invoices monetary columns DECIMAL → REAL ───────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoices_new (
                id              INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_no      TEXT(64)    NOT NULL UNIQUE,
                counterparty    TEXT        NOT NULL,
                issue_date      DATE        NOT NULL,
                subtotal        REAL        NOT NULL,
                tax             REAL        NOT NULL,
                total           REAL        NOT NULL,
                status          TEXT(16)    NOT NULL DEFAULT 'draft'
                                    CHECK ({INVOICES_STATUS_CHECK}),
                created_by      INTEGER     NOT NULL REFERENCES users(id),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL,
                tax_rate        REAL        NOT NULL DEFAULT 0,
                discount_type   TEXT,
                discount_value  REAL,
                discount_amount REAL        NOT NULL DEFAULT 0,
                CHECK ({INVOICES_TOTALS_CHECK})
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT OR IGNORE INTO invoices_new
                (id, invoice_no, counterparty, issue_date,
                 subtotal, tax, total, status, created_by, created_at, updated_at,
                 tax_rate, discount_type, discount_value, discount_amount)
             SELECT
                id, invoice_no, counterparty, issue_date,
                CAST(subtotal AS REAL), CAST(tax AS REAL), CAST(total AS REAL),
                status, created_by, created_at, updated_at,
                CAST(tax_rate AS REAL), discount_type,
                CAST(discount_value AS REAL), CAST(discount_amount AS REAL)
             FROM invoices",
        )
        .await
        .ok();

        db.execute_unprepared("DROP TABLE IF EXISTS invoices").await?;
        db.execute_unprepared("ALTER TABLE invoices_new RENAME TO invoices")
            .await
            .ok();

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoices_status ON invoices (status)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoices_counterparty ON invoices (counterparty)",
        )
        .await?;

        // ── 3. Fix invoice_lines monetary columns DECIMAL → REAL ──────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoice_lines_new (
                id                       INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id               INTEGER     NOT NULL
                                             REFERENCES invoices(id) ON DELETE CASCADE,
                description              TEXT        NOT NULL,
                pricing_model            TEXT(16)    NOT NULL CHECK ({INV_LINES_PRICING_CHECK}),
                quantity                 REAL        NOT NULL,
                unit_price               REAL        NOT NULL,
                adjustment_type          TEXT(16)    CHECK ({INV_LINES_ADJ_CHECK}),
                adjustment_is_percentage INTEGER     NOT NULL DEFAULT 0,
                adjustment_value         REAL,
                line_total               REAL        NOT NULL,
                is_refund                INTEGER     NOT NULL DEFAULT 0,
                created_at               TIMESTAMPTZ NOT NULL,
                CHECK ({INV_LINES_AMOUNTS_CHECK})
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT OR IGNORE INTO invoice_lines_new
                (id, invoice_id, description, pricing_model, quantity,
                 unit_price, adjustment_type, adjustment_is_percentage,
                 adjustment_value, line_total, is_refund, created_at)
             SELECT
                id, invoice_id, description, pricing_model, quantity,
                CAST(unit_price AS REAL), adjustment_type, adjustment_is_percentage,
                CAST(adjustment_value AS REAL), CAST(line_total AS REAL),
                is_refund, created_at
             FROM invoice_lines",
        )
        .await
        .ok();

        db.execute_unprepared("DROP TABLE IF EXISTS invoice_lines").await?;
        db.execute_unprepared("ALTER TABLE invoice_lines_new RENAME TO invoice_lines")
            .await
            .ok();

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_invoice_lines_invoice_id
             ON invoice_lines (invoice_id)",
        )
        .await?;

        // ── 4. Drop FK on results.participant_id ──────────────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS results_new (
                id              INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                event_id        INTEGER     NOT NULL REFERENCES events(id) ON DELETE CASCADE,
                participant_id  INTEGER     NOT NULL,
                attempt_no      INTEGER     NOT NULL,
                value_numeric   REAL        NOT NULL,
                unit_enum       TEXT(32)    NOT NULL CHECK ({RESULTS_UNIT_CHECK}),
                entered_by      INTEGER     NOT NULL REFERENCES users(id),
                reviewed_state  TEXT(16)    NOT NULL DEFAULT 'pending'
                                    CHECK ({RESULTS_STATE_CHECK}),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT OR IGNORE INTO results_new
                (id, event_id, participant_id, attempt_no, value_numeric,
                 unit_enum, entered_by, reviewed_state, created_at, updated_at)
             SELECT
                id, event_id, participant_id, attempt_no, value_numeric,
                unit_enum, entered_by, reviewed_state, created_at, updated_at
             FROM results",
        )
        .await
        .ok();

        db.execute_unprepared("DROP TABLE IF EXISTS results").await?;
        db.execute_unprepared("ALTER TABLE results_new RENAME TO results")
            .await
            .ok();

        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_results_event_participant_attempt
             ON results (event_id, participant_id, attempt_no)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_results_reviewed_state
             ON results (reviewed_state)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // ── Restore results with FK on participant_id ─────────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS results_old (
                id              INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                event_id        INTEGER     NOT NULL REFERENCES events(id) ON DELETE CASCADE,
                participant_id  INTEGER     NOT NULL REFERENCES users(id),
                attempt_no      INTEGER     NOT NULL,
                value_numeric   REAL        NOT NULL,
                unit_enum       TEXT(32)    NOT NULL CHECK ({RESULTS_UNIT_CHECK}),
                entered_by      INTEGER     NOT NULL REFERENCES users(id),
                reviewed_state  TEXT(16)    NOT NULL DEFAULT 'pending'
                                    CHECK ({RESULTS_STATE_CHECK}),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT INTO results_old
                (id, event_id, participant_id, attempt_no, value_numeric,
                 unit_enum, entered_by, reviewed_state, created_at, updated_at)
             SELECT
                id, event_id, participant_id, attempt_no, value_numeric,
                unit_enum, entered_by, reviewed_state, created_at, updated_at
             FROM results",
        )
        .await?;

        db.execute_unprepared("DROP TABLE results").await?;
        db.execute_unprepared("ALTER TABLE results_old RENAME TO results")
            .await?;

        db.execute_unprepared(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_results_event_participant_attempt
             ON results (event_id, participant_id, attempt_no)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_results_reviewed_state
             ON results (reviewed_state)",
        )
        .await?;

        // ── Restore invoice_lines with DECIMAL columns ────────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoice_lines_old (
                id                       INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_id               INTEGER       NOT NULL
                                             REFERENCES invoices(id) ON DELETE CASCADE,
                description              TEXT          NOT NULL,
                pricing_model            TEXT(16)      NOT NULL CHECK ({INV_LINES_PRICING_CHECK}),
                quantity                 REAL          NOT NULL,
                unit_price               DECIMAL(16,4) NOT NULL,
                adjustment_type          TEXT(16)      CHECK ({INV_LINES_ADJ_CHECK}),
                adjustment_is_percentage INTEGER       NOT NULL DEFAULT 0,
                adjustment_value         DECIMAL(16,4),
                line_total               DECIMAL(16,4) NOT NULL,
                is_refund                INTEGER       NOT NULL DEFAULT 0,
                created_at               TIMESTAMPTZ   NOT NULL,
                CHECK ({INV_LINES_AMOUNTS_CHECK})
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT INTO invoice_lines_old
                (id, invoice_id, description, pricing_model, quantity,
                 unit_price, adjustment_type, adjustment_is_percentage,
                 adjustment_value, line_total, is_refund, created_at)
             SELECT
                id, invoice_id, description, pricing_model, quantity,
                unit_price, adjustment_type, adjustment_is_percentage,
                adjustment_value, line_total, is_refund, created_at
             FROM invoice_lines",
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

        // ── Restore invoices with DECIMAL columns ─────────────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS invoices_old (
                id              INTEGER       NOT NULL PRIMARY KEY AUTOINCREMENT,
                invoice_no      TEXT(64)      NOT NULL UNIQUE,
                counterparty    TEXT          NOT NULL,
                issue_date      DATE          NOT NULL,
                subtotal        DECIMAL(16,4) NOT NULL,
                tax             DECIMAL(16,4) NOT NULL,
                total           DECIMAL(16,4) NOT NULL,
                status          TEXT(16)      NOT NULL DEFAULT 'draft'
                                    CHECK ({INVOICES_STATUS_CHECK}),
                created_by      INTEGER       NOT NULL REFERENCES users(id),
                created_at      TIMESTAMPTZ   NOT NULL,
                updated_at      TIMESTAMPTZ   NOT NULL,
                tax_rate        DECIMAL(5,4)  NOT NULL DEFAULT 0,
                discount_type   TEXT,
                discount_value  DECIMAL(16,4),
                discount_amount DECIMAL(16,4) NOT NULL DEFAULT 0,
                CHECK ({INVOICES_TOTALS_CHECK})
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT INTO invoices_old
                (id, invoice_no, counterparty, issue_date,
                 subtotal, tax, total, status, created_by, created_at, updated_at,
                 tax_rate, discount_type, discount_value, discount_amount)
             SELECT
                id, invoice_no, counterparty, issue_date,
                subtotal, tax, total, status, created_by, created_at, updated_at,
                tax_rate, discount_type, discount_value, discount_amount
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

        // ── Restore assets with DECIMAL procurement_cost ──────────────────────
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS assets_old (
                id                      INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_code              TEXT(64)     NOT NULL UNIQUE,
                category                TEXT(32)     NOT NULL CHECK ({ASSETS_CATEGORY_CHECK}),
                brand                   TEXT         NOT NULL,
                model                   TEXT         NOT NULL,
                serial_number           TEXT,
                status                  TEXT(24)     NOT NULL DEFAULT 'in_service'
                                            CHECK ({ASSETS_STATUS_CHECK}),
                owner_id                INTEGER      REFERENCES users(id),
                responsible_person_id   INTEGER      REFERENCES users(id),
                procurement_cost        DECIMAL(16,4),
                procurement_date        TEXT,
                useful_life_months      INTEGER,
                notes                   TEXT,
                created_at              TIMESTAMPTZ  NOT NULL,
                updated_at              TIMESTAMPTZ  NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT INTO assets_old
                (id, asset_code, category, brand, model, serial_number,
                 status, owner_id, responsible_person_id,
                 procurement_cost, procurement_date, useful_life_months,
                 notes, created_at, updated_at)
             SELECT
                id, asset_code, category, brand, model, serial_number,
                status, owner_id, responsible_person_id,
                procurement_cost, procurement_date, useful_life_months,
                notes, created_at, updated_at
             FROM assets",
        )
        .await?;

        db.execute_unprepared("DROP TABLE assets").await?;
        db.execute_unprepared("ALTER TABLE assets_old RENAME TO assets")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_assets_category ON assets (category)",
        )
        .await?;
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_assets_status ON assets (status)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
