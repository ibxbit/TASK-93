use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use sha2::{Digest, Sha256};
use chrono::{Duration, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set,
    TransactionTrait,
};
use uuid::Uuid;

use crate::errors::{AppError, AppResult};
use crate::rbac::{
    entity::{role, role_assignment},
    Role,
};

use super::{
    entity::{session, user},
    AuthenticatedUser,
};

const SESSION_MINUTES: i64 = 30;

// ── Public output types ───────────────────────────────────────────────────────

pub struct LoginResult {
    pub token: String,
    pub expires_at: chrono::DateTime<Utc>,
}

// ── Login ─────────────────────────────────────────────────────────────────────

pub async fn login(
    conn: &DatabaseConnection,
    username: &str,
    password: &str,
) -> AppResult<LoginResult> {
    // 1. Locate user — same error for unknown user vs wrong password (no enumeration).
    let user_model = user::Entity::find()
        .filter(user::Column::Username.eq(username))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Invalid credentials".into()))?;

    // 2. Verify password against stored Argon2 hash.
    let parsed = PasswordHash::new(&user_model.password_hash)
        .map_err(|e| AppError::Internal(format!("hash parse: {e}")))?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized("Invalid credentials".into()))?;

    // 3. Stamp last_login_at.
    let mut active: user::ActiveModel = user_model.clone().into();
    active.last_login_at = Set(Some(Utc::now()));
    active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // 4. Create session with 30-min expiry.
    //    The raw token is returned to the client; only its SHA-256 hash is stored.
    let token = new_token();
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let expires_at = Utc::now() + Duration::minutes(SESSION_MINUTES);

    session::ActiveModel {
        token_hash: Set(token_hash),
        user_id: Set(user_model.id),
        expires_at: Set(expires_at),
        created_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        user_id  = user_model.id,
        username = %username,
        "auth.login.success",
    );

    Ok(LoginResult { token, expires_at })
}

// ── Logout ────────────────────────────────────────────────────────────────────

pub async fn logout(conn: &DatabaseConnection, token: &str) -> AppResult<()> {
    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    session::Entity::delete_many()
        .filter(session::Column::TokenHash.eq(token_hash))
        .exec(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!("auth.logout.success");
    Ok(())
}

// ── Password rotation ─────────────────────────────────────────────────────────

pub async fn rotate_password(
    conn: &DatabaseConnection,
    user_id: i64,
    current_password: &str,
    new_password: &str,
) -> AppResult<()> {
    // 1. Load user and verify current password.
    let user_model = user::Entity::find_by_id(user_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("user not found".into()))?;

    let parsed = PasswordHash::new(&user_model.password_hash)
        .map_err(|e| AppError::Internal(format!("hash parse: {e}")))?;
    Argon2::default()
        .verify_password(current_password.as_bytes(), &parsed)
        .map_err(|_| AppError::Unauthorized("Current password is incorrect".into()))?;

    // 2. Hash the new password.
    let salt = SaltString::generate(&mut OsRng);
    let new_hash = Argon2::default()
        .hash_password(new_password.as_bytes(), &salt)
        .map_err(|e| AppError::Internal(format!("hash error: {e}")))?
        .to_string();

    // 3. Atomically update password + purge all sessions → forces re-login
    //    on all devices, preventing use of stolen tokens post-rotation.
    conn.transaction::<_, (), sea_orm::DbErr>(|txn| {
        let new_hash = new_hash.clone();
        Box::pin(async move {
            let mut active: user::ActiveModel = user_model.into();
            active.password_hash = Set(new_hash);
            active.update(txn).await?;

            session::Entity::delete_many()
                .filter(session::Column::UserId.eq(user_id))
                .exec(txn)
                .await?;

            Ok(())
        })
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(user_id, "auth.password_rotated");
    Ok(())
}

// ── Session validation (used by the request guard) ────────────────────────────

/// Looks up the session by token, rejects if expired, then slides the expiry
/// window forward by `SESSION_MINUTES` before returning the authenticated user.
pub async fn validate_session(
    conn: &DatabaseConnection,
    token: &str,
) -> AppResult<AuthenticatedUser> {
    let now = Utc::now();

    let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
    let session_model = session::Entity::find()
        .filter(session::Column::TokenHash.eq(token_hash))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Unauthorized("Session not found".into()))?;

    if session_model.expires_at <= now {
        // Clean up the stale row; ignore errors — best-effort housekeeping.
        session::Entity::delete_by_id(session_model.id)
            .exec(conn)
            .await
            .ok();
        return Err(AppError::Unauthorized("Session expired".into()));
    }

    // Slide the expiry window forward (inactivity timeout).
    let new_expires = now + Duration::minutes(SESSION_MINUTES);
    let mut active: session::ActiveModel = session_model.clone().into();
    active.expires_at = Set(new_expires);
    active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Load the associated user for the guard payload.
    let user_model = user::Entity::find_by_id(session_model.user_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("session references missing user".into()))?;

    // Load the user's current roles (two small indexed queries).
    let assignments = role_assignment::Entity::find()
        .filter(role_assignment::Column::UserId.eq(user_model.id))
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let roles: Vec<Role> = if assignments.is_empty() {
        vec![]
    } else {
        let role_ids: Vec<i64> = assignments.iter().map(|a| a.role_id).collect();
        role::Entity::find()
            .filter(role::Column::Id.is_in(role_ids))
            .all(conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .iter()
            .filter_map(|r| Role::from_str(&r.name))
            .collect()
    };

    Ok(AuthenticatedUser {
        user_id: user_model.id,
        username: user_model.username,
        session_token: token.to_owned(),
        roles,
    })
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Generates a 64-character hex session token using two UUIDv4 values.
/// Provides 122 bits of cryptographic randomness — sufficient for session tokens.
fn new_token() -> String {
    format!(
        "{}{}",
        Uuid::new_v4().as_simple(),
        Uuid::new_v4().as_simple()
    )
}
