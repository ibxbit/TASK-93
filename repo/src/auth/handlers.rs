use crate::middleware::rate_limit::RateLimitedToken;
use rocket::{serde::json::Json, Request, State};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};

use crate::errors::{AppResult, ErrorBody};
use crate::middleware::correlation::CorrelationId;

use super::{service, AuthenticatedUser};

// ── Request / response bodies ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    /// ISO 8601 timestamp with UTC offset.
    pub expires_at: String,
}

#[derive(Deserialize)]
pub struct RotatePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct OkResponse {
    ok: bool,
}

// ── Route handlers ────────────────────────────────────────────────────────────

/// Authenticate with username + password.  Returns a bearer token valid for
/// 30 minutes of inactivity.  Timestamps `last_login_at` on success.
#[post("/auth/login", data = "<body>")]
pub async fn login(
    body: Json<LoginRequest>,
    _rate_limit: RateLimitedToken,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<LoginResponse>> {
    let result = service::login(conn.inner(), &body.username, &body.password).await?;
    Ok(Json(LoginResponse {
        token: result.token,
        expires_at: result.expires_at.to_rfc3339(),
    }))
}

/// Invalidate the caller's current session.  Subsequent requests with the
/// same token will receive HTTP 401.
#[post("/auth/logout")]
pub async fn logout(
    user: AuthenticatedUser,
    _rate_limit: RateLimitedToken,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<OkResponse>> {
    service::logout(conn.inner(), &user.session_token).await?;
    Ok(Json(OkResponse { ok: true }))
}

/// Change the authenticated user's password.
///
/// On success:
/// - All sessions for the user are immediately invalidated.
/// - The caller must re-authenticate with the new password.
#[post("/auth/rotate-password", data = "<body>")]
pub async fn rotate_password(
    user: AuthenticatedUser,
    body: Json<RotatePasswordRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<OkResponse>> {
    service::rotate_password(
        conn.inner(),
        user.user_id,
        &body.current_password,
        &body.new_password,
    )
    .await?;
    Ok(Json(OkResponse { ok: true }))
}

// ── Catchers ──────────────────────────────────────────────────────────────────

/// Converts Rocket's 401 Unauthorized into a structured JSON response that
/// includes the request correlation ID for log correlation.
#[catch(401)]
pub fn catch_unauthorized(req: &Request) -> Json<ErrorBody> {
    let cid = req.local_cache(|| CorrelationId(String::new())).0.clone();
    Json(ErrorBody {
        code: "UNAUTHORIZED",
        message: "Authentication required or session expired".to_string(),
        correlation_id: if cid.is_empty() { None } else { Some(cid) },
    })
}
