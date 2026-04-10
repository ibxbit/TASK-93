#[macro_use]
extern crate rocket;

mod analytics;
mod assets;
mod audit;
mod auth;
mod backup;
mod billing;
mod competition;
mod config;
mod crypto;
mod data_quality;
mod db;
mod entity;
mod errors;
mod middleware;
mod migration;
mod payments;
mod rbac;
mod results;
mod vehicles;

use rocket::serde::json::Json;
use serde::Serialize;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use analytics::handlers::{
    create_metric, export, funnel, get_metric, list_metrics, retention, trends, update_metric,
};
use assets::handlers::{
    create_asset, export_assets, get_asset, get_history, import_assets, list_assets, update_asset,
    update_status,
};
use audit::handlers::{get_audit_log, list_audit_logs};
use auth::handlers::{catch_unauthorized, login, logout, rotate_password};
use backup::handlers::{list_backups, restore_backup, trigger_backup};
use billing::handlers::{
    add_line, apply_discount, create_invoice, get_invoice, issue_invoice, list_invoices,
};
use competition::handlers::{
    create_event, create_ruleset, get_event, get_ruleset, list_events, list_rulesets,
    publish_event, rollback_ruleset, update_event,
};
use config::AppConfig;
use crypto::Cipher;
use data_quality::handlers::{get_scan, list_scans, run_scan};
use middleware::{correlation::CorrelationFairing, logger::RequestLoggerFairing};
use payments::handlers::{
    approve_refund, handle_exception, list_exceptions, list_payments, record_payment,
    reject_refund, request_refund,
};
use rbac::handlers::{assign_role, catch_forbidden, list_user_roles, revoke_role};
use results::handlers::{
    arbitrate_result, export_results, get_rankings, list_corrections, list_reviews,
    request_correction, resolve_correction, submit_result, submit_review,
};
use vehicles::handlers::{
    create_vehicle, get_history as get_vehicle_history, get_vehicle, list_vehicles,
    transition_status, update_vehicle,
};

// ── Custom error catchers ─────────────────────────────────────────────────────

/// Convert Rocket's default HTML 422 response into a structured JSON body.
/// Triggered when JSON deserialization of the request body fails (e.g. missing
/// required field) before the route handler is even called.
#[catch(422)]
fn catch_unprocessable(req: &rocket::Request<'_>) -> Json<errors::ErrorBody> {
    let cid = req
        .local_cache(|| middleware::correlation::CorrelationId(String::new()))
        .0
        .clone();
    Json(errors::ErrorBody {
        code: "UNPROCESSABLE_ENTITY",
        message: "The request body could not be parsed or contains invalid values.".to_string(),
        correlation_id: if cid.is_empty() { None } else { Some(cid) },
    })
}

// ── Health check ──────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[get("/health")]
fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
    })
}

// ── Entrypoint ────────────────────────────────────────────────────────────────

#[rocket::main]
async fn main() -> Result<(), rocket::Error> {
    dotenvy::dotenv().ok();

    // ── Config ────────────────────────────────────────────────────────────────
    let app_config = AppConfig::from_env().unwrap_or_else(|e| {
        eprintln!("FATAL configuration error: {e}");
        std::process::exit(1);
    });

    // ── Structured logging ────────────────────────────────────────────────────
    tracing_subscriber::registry()
        .with(EnvFilter::new(&app_config.log_level))
        .with(fmt::layer().json().flatten_event(true))
        .init();

    // ── Crypto key validation ─────────────────────────────────────────────────
    let cipher = Cipher::from_base64_key(&app_config.encryption_key).unwrap_or_else(|e| {
        tracing::error!(
            error = %e,
            "FATAL: encryption key validation failed. \
            The ENCRYPTION_KEY environment variable is missing or invalid. \
            You must generate a 32-byte Base64-encoded key and set it in your .env file as: \
            ENCRYPTION_KEY=\"<base64-encoded-32-byte-key>\". \
            To generate one, run: \
            openssl rand -base64 32 \
            or (Python): python3 -c \"import secrets, base64; print(base64.b64encode(secrets.token_bytes(32)).decode())\"",
        );
        std::process::exit(1);
    });

    // ── Database ──────────────────────────────────────────────────────────────
    let conn = db::connect(&app_config).await.unwrap_or_else(|e| {
        tracing::error!(
            error = %e,
            "FATAL: database connection failed. \
            Check that DATABASE_URL in your .env file points to a valid SQLite path \
            and that the target directory (e.g. data/) exists and is writable by this process. \
            You can create the directory with: mkdir -p data && chmod 755 data",
        );
        std::process::exit(1);
    });

    // ── Seed roles ────────────────────────────────────────────────────────────
    rbac::seeder::seed_roles(&conn).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "FATAL: role seeding failed");
        std::process::exit(1);
    });

    // ── Seed default users ────────────────────────────────────────────────────
    auth::seeder::seed_users(&conn).await.unwrap_or_else(|e| {
        tracing::error!(error = %e, "FATAL: user seeding failed");
        std::process::exit(1);
    });

    // ── Nightly backup scheduler ──────────────────────────────────────────────
    {
        use std::path::PathBuf;
        use std::sync::Arc;
        backup::scheduler::start(
            Arc::new(conn.clone()),
            PathBuf::from(&app_config.backup_dir),
            app_config.backup_retain_days,
        );
    }

    tracing::info!(
        version   = env!("CARGO_PKG_VERSION"),
        host      = %app_config.host,
        port      = app_config.port,
        log_level = %app_config.log_level,
        "motorsport-backend starting",
    );

    // ── Rocket ────────────────────────────────────────────────────────────────
    let rocket_cfg = rocket::Config {
        address: app_config
            .host
            .parse()
            .unwrap_or_else(|_| "0.0.0.0".parse().unwrap()),
        port: app_config.port,
        log_level: rocket::config::LogLevel::Off,
        ..rocket::Config::default()
    };

    rocket::custom(rocket_cfg)
        .attach(CorrelationFairing)
        .attach(RequestLoggerFairing)
        .manage(app_config)
        .manage(cipher)
        .manage(conn)
        .mount("/", routes![health, login, logout, rotate_password])
        .mount("/", routes![assign_role, revoke_role, list_user_roles])
        .mount(
            "/",
            routes![
                create_event,
                get_event,
                list_events,
                update_event,
                publish_event,
                create_ruleset,
                get_ruleset,
                list_rulesets,
                rollback_ruleset,
            ],
        )
        .mount("/", routes![submit_result, get_rankings, export_results])
        .mount(
            "/",
            routes![
                create_vehicle,
                get_vehicle,
                list_vehicles,
                update_vehicle,
                transition_status,
                get_vehicle_history,
            ],
        )
        .mount(
            "/",
            routes![
                create_asset,
                get_asset,
                list_assets,
                update_asset,
                update_status,
                get_history,
                export_assets,
                import_assets,
            ],
        )
        .mount("/", routes![submit_review, list_reviews, arbitrate_result])
        .mount(
            "/",
            routes![request_correction, list_corrections, resolve_correction],
        )
        .mount(
            "/",
            routes![
                create_invoice,
                get_invoice,
                list_invoices,
                add_line,
                apply_discount,
                issue_invoice,
            ],
        )
        .mount(
            "/",
            routes![
                record_payment,
                list_payments,
                handle_exception,
                list_exceptions,
                request_refund,
                approve_refund,
                reject_refund,
            ],
        )
        .mount(
            "/",
            routes![
                create_metric,
                list_metrics,
                get_metric,
                update_metric,
                trends,
                funnel,
                retention,
                export,
            ],
        )
        .mount("/", routes![run_scan, list_scans, get_scan])
        .mount("/", routes![list_audit_logs, get_audit_log])
        .mount("/", routes![list_backups, trigger_backup, restore_backup])
        .register("/", catchers![catch_unauthorized, catch_forbidden, catch_unprocessable])
        .launch()
        .await?;

    Ok(())
}
