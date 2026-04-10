use rocket::{
    http::Status,
    request::{FromRequest, Outcome},
    Request, State,
};
use sea_orm::DatabaseConnection;

use super::{service, AuthenticatedUser};

/// Extracts and validates the `Authorization: Bearer <token>` header.
///
/// On success, extends the session's `expires_at` (sliding 30-min window)
/// before returning the populated `AuthenticatedUser`.
///
/// Returns HTTP 401 for any of:
/// - Missing / malformed Authorization header
/// - Token not found in the sessions table
/// - Session past its `expires_at`
#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthenticatedUser {
    type Error = ();

    async fn from_request(req: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // Extract and strip the "Bearer " prefix.
        let token = match req
            .headers()
            .get_one("Authorization")
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            Some(t) => t.to_owned(),
            None => return Outcome::Error((Status::Unauthorized, ())),
        };

        let conn = match req.guard::<&State<DatabaseConnection>>().await {
            Outcome::Success(c) => c,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        match service::validate_session(conn.inner(), &token).await {
            Ok(user) => Outcome::Success(user),
            Err(_) => Outcome::Error((Status::Unauthorized, ())),
        }
    }
}
