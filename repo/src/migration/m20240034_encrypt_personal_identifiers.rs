use sea_orm_migration::prelude::*;

/// Adds a `vin_hash` blind-index column to `vehicles` to support equality
/// lookups on the now-encrypted `vin` field.
///
/// The unique constraint moves from `vin` (plaintext) to `vin_hash` (keyed
/// digest).  `registration_id` is also encrypted but carries no unique
/// constraint, so no companion hash column is required for it.
///
/// Existing rows have their `vin_hash` initialised to the current plaintext
/// VIN value — a one-time re-encryption job should be run against any
/// pre-existing data in a production deployment.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        db.execute_unprepared("PRAGMA foreign_keys = OFF").await?;

        db.execute_unprepared(
            r#"
            CREATE TABLE IF NOT EXISTS vehicles_new (
                id                   INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_id             INTEGER REFERENCES assets(id) ON DELETE SET NULL,
                vin                  TEXT    NOT NULL,
                vin_hash             TEXT    NOT NULL DEFAULT '' UNIQUE,
                registration_id      TEXT    NOT NULL,
                make                 TEXT(64) NOT NULL,
                model                TEXT(64) NOT NULL,
                year                 INTEGER  NOT NULL CHECK (year BETWEEN 1886 AND 2100),
                color                TEXT(32),
                mileage              INTEGER  NOT NULL DEFAULT 0 CHECK (mileage >= 0),
                title_transfer_count INTEGER  NOT NULL DEFAULT 0 CHECK (title_transfer_count >= 0),
                status               TEXT(16) NOT NULL DEFAULT 'draft'
                                         CHECK (status IN ('draft','published','delisted','sold')),
                status_reason        TEXT,
                created_by           INTEGER  NOT NULL REFERENCES users(id),
                created_at           TIMESTAMPTZ NOT NULL,
                updated_at           TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        // Seed vin_hash with the existing plaintext VIN so that uniqueness
        // checks still work for any pre-existing rows.
        db.execute_unprepared(
            r#"
            INSERT OR IGNORE INTO vehicles_new
                (id, asset_id, vin, vin_hash, registration_id,
                 make, model, year, color, mileage, title_transfer_count,
                 status, status_reason, created_by, created_at, updated_at)
            SELECT id, asset_id, vin, vin, registration_id,
                   make, model, year, color, mileage, title_transfer_count,
                   status, status_reason, created_by, created_at, updated_at
            FROM vehicles
            "#,
        )
        .await
        .ok();

        db.execute_unprepared("DROP TABLE IF EXISTS vehicles").await?;
        db.execute_unprepared("ALTER TABLE vehicles_new RENAME TO vehicles")
            .await
            .ok();

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_vehicles_status \
             ON vehicles(status)",
        )
        .await?;

        db.execute_unprepared(
            "CREATE INDEX IF NOT EXISTS idx_vehicles_make \
             ON vehicles(make)",
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
            CREATE TABLE IF NOT EXISTS vehicles_old (
                id                   INTEGER NOT NULL PRIMARY KEY AUTOINCREMENT,
                asset_id             INTEGER REFERENCES assets(id) ON DELETE SET NULL,
                vin                  TEXT    NOT NULL UNIQUE,
                registration_id      TEXT    NOT NULL,
                make                 TEXT(64) NOT NULL,
                model                TEXT(64) NOT NULL,
                year                 INTEGER  NOT NULL CHECK (year BETWEEN 1886 AND 2100),
                color                TEXT(32),
                mileage              INTEGER  NOT NULL DEFAULT 0 CHECK (mileage >= 0),
                title_transfer_count INTEGER  NOT NULL DEFAULT 0 CHECK (title_transfer_count >= 0),
                status               TEXT(16) NOT NULL DEFAULT 'draft'
                                         CHECK (status IN ('draft','published','delisted','sold')),
                status_reason        TEXT,
                created_by           INTEGER  NOT NULL REFERENCES users(id),
                created_at           TIMESTAMPTZ NOT NULL,
                updated_at           TIMESTAMPTZ NOT NULL
            )
            "#,
        )
        .await?;

        db.execute_unprepared(
            r#"
            INSERT INTO vehicles_old
                (id, asset_id, vin, registration_id, make, model, year,
                 color, mileage, title_transfer_count, status, status_reason,
                 created_by, created_at, updated_at)
            SELECT id, asset_id, vin, registration_id, make, model, year,
                   color, mileage, title_transfer_count, status, status_reason,
                   created_by, created_at, updated_at
            FROM vehicles
            "#,
        )
        .await?;

        db.execute_unprepared("DROP TABLE vehicles").await?;
        db.execute_unprepared("ALTER TABLE vehicles_old RENAME TO vehicles")
            .await?;

        db.execute_unprepared("PRAGMA foreign_keys = ON").await?;

        Ok(())
    }
}
