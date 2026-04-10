use std::path::PathBuf;
use std::time::Duration;

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};
use sea_orm_migration::MigratorTrait;

use crate::backup::service as backup_service;
use crate::config::AppConfig;
use crate::migration::Migrator;

/// Open the SQLite connection pool and run all pending migrations.
///
/// Before opening any connections this function checks whether a restore was
/// staged by a previous run (via `POST /admin/backups/<filename>/restore`).
/// If a `.restore_pending` marker is found the backup is copied over the live
/// database file and the marker is deleted — restoring the database to the
/// backed-up state before any application code touches it.
///
/// # Migration isolation (WAL / multi-connection fix)
///
/// Sea-ORM's `execute_unprepared` acquires a connection from the pool for
/// every call.  With `max_connections > 1` and SQLite WAL mode, the
/// `DROP TABLE X` and the immediately following `ALTER TABLE X_new RENAME TO X`
/// can land on *different* pool connections.  Connection B may hold a WAL read
/// snapshot that was opened before connection A's DROP committed, so B still
/// "sees" the old table name → the RENAME fails with "table already exists",
/// which is swallowed by `.ok()`, leaving `X` dropped and `X_new` un-renamed.
/// The subsequent index creation on the (now-absent) `X` then raises
/// "no such table: main.X" and the process exits.
///
/// Running migrations through a dedicated **single-connection** pool ensures
/// every statement within one migration sees the same committed state and
/// eliminates the snapshot divergence.  The migration pool is explicitly
/// closed (checkpointing the WAL) before the application pool is opened.
pub async fn connect(config: &AppConfig) -> Result<DatabaseConnection, sea_orm::DbErr> {
    // ── Ensure database directory and file exist ───────────────────────────────
    // sqlx/SQLite does not create the database file automatically; the
    // directory must also exist when running on a fresh volume mount.
    if let Some(db_file) = backup_service::db_path_from_url(&config.database_url) {
        if let Some(parent) = db_file.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                sea_orm::DbErr::Custom(format!("Cannot create database directory: {e}"))
            })?;
        }
        if !db_file.exists() {
            std::fs::File::create(&db_file).map_err(|e| {
                sea_orm::DbErr::Custom(format!("Cannot initialise database file: {e}"))
            })?;
        }
    }

    // ── Pending restore check ─────────────────────────────────────────────────
    let backup_dir = PathBuf::from(&config.backup_dir);
    if let Some(db_file) = backup_service::db_path_from_url(&config.database_url) {
        match backup_service::apply_pending_restore(&backup_dir, &db_file) {
            Ok(true) => {
                tracing::info!(
                    db_file = %db_file.display(),
                    "database.restore_applied — proceeding with restored database"
                );
            }
            Ok(false) => {} // No restore pending — normal startup.
            Err(e) => {
                // A failed restore is fatal: we do not know if the database
                // file is in a consistent state.
                return Err(sea_orm::DbErr::Custom(format!(
                    "Staged restore failed at startup: {e}"
                )));
            }
        }
    }

    // ── Migrations via a dedicated single-connection pool ─────────────────────
    // max_connections(1) guarantees that every execute_unprepared call within
    // a migration uses the *same* physical SQLite connection.  This eliminates
    // the WAL read-snapshot divergence described above.
    {
        let mut mig_opts = ConnectOptions::new(&config.database_url);
        mig_opts
            .max_connections(1)
            .min_connections(1)
            .connect_timeout(Duration::from_secs(30))
            .acquire_timeout(Duration::from_secs(30))
            .idle_timeout(Duration::from_secs(300))
            .sqlx_logging(false);

        let mig_conn = Database::connect(mig_opts).await?;

        tracing::info!(url = %config.database_url, "database.connecting");

        // Enable WAL mode and a generous busy timeout before migrations touch
        // the schema.  The full PRAGMA set is applied again on the app pool
        // after migrations complete.
        for pragma in &[
            "PRAGMA journal_mode = WAL",
            "PRAGMA synchronous = NORMAL",
            "PRAGMA busy_timeout = 5000",
        ] {
            mig_conn
                .execute(Statement::from_string(DbBackend::Sqlite, pragma.to_string()))
                .await?;
        }

        Migrator::up(&mig_conn, None).await?;

        tracing::info!("database.migrations_applied");

        // Explicitly close the migration pool so SQLite checkpoints the WAL
        // file before the application pool opens its own connections.
        mig_conn.close().await.ok();
    }

    // ── Application connection pool ───────────────────────────────────────────
    let mut opts = ConnectOptions::new(&config.database_url);
    opts.max_connections(5)
        .min_connections(1)
        .connect_timeout(Duration::from_secs(5))
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(300))
        .sqlx_logging(false); // query-level logging handled by our tracing layer

    let conn = Database::connect(opts).await?;

    // ── SQLite performance PRAGMAs ────────────────────────────────────────────
    // Applied once after the pool opens; effective for every subsequent
    // connection acquired from the pool via the per-connection PRAGMA hook.
    //
    // synchronous = NORMAL — skips the full-fsync on every commit while still
    //   providing crash-safe writes in WAL mode.  Dramatically reduces latency
    //   on spinning disks and networked volumes.
    //
    // cache_size = -64000 — 64 MiB page cache (negative = kibibytes).  Keeps
    //   hot pages in memory and avoids repeat reads for the primary hot tables
    //   (invoices, payments, audit_log, vehicles).
    //
    // temp_store = MEMORY — stores temporary B-trees and sort buffers in RAM
    //   rather than on disk.  Benefits the analytics queries and ranking
    //   computations that sort large intermediate result sets.
    //
    // mmap_size = 268435456 — 256 MiB memory-mapped I/O window.  Sequential
    //   scans (data-quality checks, audit log queries) avoid read() system
    //   calls; pages are faulted directly into the process address space.
    //
    // busy_timeout = 5000 — wait up to 5 s before returning SQLITE_BUSY when
    //   a writer holds the WAL write lock.  Prevents spurious 500 errors under
    //   brief write contention.
    for pragma in &[
        "PRAGMA journal_mode = WAL",
        "PRAGMA synchronous = NORMAL",
        "PRAGMA cache_size = -64000",
        "PRAGMA temp_store = MEMORY",
        "PRAGMA mmap_size = 268435456",
        "PRAGMA busy_timeout = 5000",
    ] {
        conn.execute(Statement::from_string(
            DbBackend::Sqlite,
            pragma.to_string(),
        ))
        .await?;
    }

    tracing::info!("database.pragmas_applied");

    Ok(conn)
}
