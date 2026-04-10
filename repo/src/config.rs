use std::env;

/// Central application configuration loaded from environment variables.
/// All sensitive fields (ENCRYPTION_KEY) are validated at startup.
#[derive(Debug, Clone)]
pub struct AppConfig {
    /// SQLite connection string, e.g. `sqlite:///app/data/motorsport.db`
    pub database_url: String,

    /// Base64-encoded 32-byte AES-256 key.  Validated by the crypto module
    /// on startup — the app will refuse to launch if the key is absent or
    /// malformed.
    pub encryption_key: String,

    /// Bind address (default: 0.0.0.0)
    pub host: String,

    /// Listen port (default: 8000)
    pub port: u16,

    /// tracing env-filter directive, e.g. `info`, `debug`, `motorsport_backend=trace`
    pub log_level: String,

    /// Number of nightly backup files to retain (default: 7)
    pub backup_retain_days: u32,

    /// Local directory where backup files are stored.  Created on first
    /// backup run if it does not exist.  (default: `./backups`)
    pub backup_dir: String,
}

impl AppConfig {
    /// Build config from environment variables.  Returns `Err` if any required
    /// variable is missing or has an invalid value.
    pub fn from_env() -> Result<Self, String> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite:///app/data/motorsport.db".to_string()),

            encryption_key: env::var("ENCRYPTION_KEY").map_err(|_| {
                "ENCRYPTION_KEY is required but not set. \
                 Generate a value with: openssl rand -base64 32 \
                 and set it as an environment variable or in your .env file."
                    .to_string()
            })?,

            host: env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string()),

            port: env::var("PORT")
                .unwrap_or_else(|_| "8000".to_string())
                .parse::<u16>()
                .map_err(|_| "PORT must be a valid u16 (1–65535)".to_string())?,

            log_level: env::var("LOG_LEVEL").unwrap_or_else(|_| "info".to_string()),

            backup_retain_days: env::var("BACKUP_RETAIN_DAYS")
                .unwrap_or_else(|_| "7".to_string())
                .parse::<u32>()
                .unwrap_or(7),

            backup_dir: env::var("BACKUP_DIR").unwrap_or_else(|_| "./backups".to_string()),
        })
    }
}
