use sea_orm_migration::prelude::*;

/// Recreate `assets` with full ledger columns:
///   - Rename `depreciation_months` → `useful_life_months`
///   - Add `status`, `owner_id`, `responsible_person_id`,
///     `procurement_cost`, `procurement_date`
///
/// SQLite does not support ALTER COLUMN or multi-column ADD COLUMN in one
/// statement, so the standard table-recreation pattern is used with FK
/// enforcement temporarily disabled.
#[derive(DeriveMigrationName)]
pub struct Migration;

const CATEGORY_CHECK: &str =
    "category IN ('vehicle', 'equipment', 'facility', 'electronic', 'other')";

const STATUS_CHECK: &str = "status IN ('in_service', 'out_for_repair', 'retired')";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        // 1. Create replacement table with full column set.
        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS assets_new (
                id                      INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_code              TEXT(64)     NOT NULL UNIQUE,
                category                TEXT(32)     NOT NULL CHECK ({CATEGORY_CHECK}),
                brand                   TEXT         NOT NULL,
                model                   TEXT         NOT NULL,
                serial_number           TEXT,
                status                  TEXT(24)     NOT NULL DEFAULT 'in_service'
                                            CHECK ({STATUS_CHECK}),
                owner_id                INTEGER      REFERENCES users(id),
                responsible_person_id   INTEGER      REFERENCES users(id),
                procurement_cost        DECIMAL(19,4),
                procurement_date        TEXT,
                useful_life_months      INTEGER,
                notes                   TEXT,
                created_at              TIMESTAMPTZ  NOT NULL,
                updated_at              TIMESTAMPTZ  NOT NULL
            )"
        ))
        .await?;

        // 2. Migrate existing rows; rename depreciation_months → useful_life_months.
        db.execute_unprepared(
            "INSERT INTO assets_new
                (id, asset_code, category, brand, model, serial_number,
                 status, owner_id, responsible_person_id,
                 procurement_cost, procurement_date, useful_life_months,
                 notes, created_at, updated_at)
             SELECT
                id, asset_code, category, brand, model, serial_number,
                'in_service', NULL, NULL,
                NULL, NULL, depreciation_months,
                notes, created_at, updated_at
             FROM assets",
        )
        .await?;

        // 3. Swap tables.
        db.execute_unprepared("DROP TABLE assets").await?;
        db.execute_unprepared("ALTER TABLE assets_new RENAME TO assets")
            .await?;

        // 4. Recreate indexes.
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_assets_category ON assets (category)",
        )
        .await?;
        db.execute_unprepared("CREATE INDEX IF NOT EXISTS idx_assets_status ON assets (status)")
            .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS assets_old (
                id                  INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_code          TEXT(64)    NOT NULL UNIQUE,
                category            TEXT(32)    NOT NULL CHECK ({CATEGORY_CHECK}),
                brand               TEXT        NOT NULL,
                model               TEXT        NOT NULL,
                serial_number       TEXT,
                depreciation_months INTEGER     NOT NULL DEFAULT 0,
                notes               TEXT,
                created_at          TIMESTAMPTZ NOT NULL,
                updated_at          TIMESTAMPTZ NOT NULL
            )"
        ))
        .await?;

        db.execute_unprepared(
            "INSERT INTO assets_old
                (id, asset_code, category, brand, model, serial_number,
                 depreciation_months, notes, created_at, updated_at)
             SELECT
                id, asset_code, category, brand, model, serial_number,
                COALESCE(useful_life_months, 0), notes, created_at, updated_at
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

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
