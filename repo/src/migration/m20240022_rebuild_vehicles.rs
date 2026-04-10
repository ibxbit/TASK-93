use sea_orm_migration::prelude::*;

/// Replace the original `vehicles` table (operational statuses) with the full
/// lifecycle model: new status enum, mileage, title_transfer_count, make,
/// model, year, color, status_reason, created_by.
///
/// SQLite cannot ALTER TABLE to change a CHECK constraint or add NOT NULL
/// columns without defaults, so we use the standard table-swap pattern with
/// FK enforcement temporarily disabled.
#[derive(DeriveMigrationName)]
pub struct Migration;

const STATUS_CHECK: &str = "status IN ('draft', 'published', 'delisted', 'sold')";

const TITLE_CHECK: &str = "title_transfer_count >= 0";

const MILEAGE_CHECK: &str = "mileage >= 0";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(&format!(
            "CREATE TABLE IF NOT EXISTS vehicles_new (
                id                   INTEGER      NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_id             INTEGER      REFERENCES assets(id) ON DELETE SET NULL,
                vin                  TEXT(17)     NOT NULL UNIQUE,
                registration_id      TEXT         NOT NULL,
                make                 TEXT         NOT NULL DEFAULT '',
                model                TEXT         NOT NULL DEFAULT '',
                year                 INTEGER      NOT NULL DEFAULT 0,
                color                TEXT,
                mileage              INTEGER      NOT NULL DEFAULT 0 CHECK ({MILEAGE_CHECK}),
                title_transfer_count INTEGER      NOT NULL DEFAULT 0 CHECK ({TITLE_CHECK}),
                status               TEXT(16)     NOT NULL DEFAULT 'draft' CHECK ({STATUS_CHECK}),
                status_reason        TEXT,
                created_by           INTEGER      NOT NULL DEFAULT 0 REFERENCES users(id),
                created_at           TIMESTAMPTZ  NOT NULL,
                updated_at           TIMESTAMPTZ  NOT NULL
            )"
        ))
        .await?;

        // Carry forward existing VIN, registration_id, asset_id — map old
        // operational statuses to 'draft' since there is no meaningful mapping.
        db.execute_unprepared(
            "INSERT INTO vehicles_new
                (id, asset_id, vin, registration_id,
                 make, model, year, color,
                 mileage, title_transfer_count,
                 status, status_reason, created_by, created_at, updated_at)
             SELECT
                id, asset_id, vin, registration_id,
                '', '', 0, NULL,
                0, 0,
                'draft', NULL, 0, created_at, updated_at
             FROM vehicles",
        )
        .await?;

        db.execute_unprepared("DROP TABLE vehicles").await?;
        db.execute_unprepared("ALTER TABLE vehicles_new RENAME TO vehicles")
            .await?;

        // Recreate the status index (was previously on old status values).
        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_vehicles_status ON vehicles (status)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS vehicles_old (
                id              INTEGER     NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_id        INTEGER     REFERENCES assets(id) ON DELETE SET NULL,
                vin             TEXT(17)    NOT NULL UNIQUE,
                registration_id TEXT        NOT NULL,
                status          TEXT(24)    NOT NULL DEFAULT 'active'
                                    CHECK (status IN ('active','inactive','retired','under_maintenance')),
                created_at      TIMESTAMPTZ NOT NULL,
                updated_at      TIMESTAMPTZ NOT NULL
            )",
        )
        .await?;

        db.execute_unprepared(
            "INSERT INTO vehicles_old
                (id, asset_id, vin, registration_id, status, created_at, updated_at)
             SELECT
                id, asset_id, vin, registration_id, 'active', created_at, updated_at
             FROM vehicles",
        )
        .await?;

        db.execute_unprepared("DROP TABLE vehicles").await?;
        db.execute_unprepared("ALTER TABLE vehicles_old RENAME TO vehicles")
            .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_vehicles_status ON vehicles (status)",
        )
        .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
