pub mod entity;
pub mod guards;
pub mod handlers;
pub mod seeder;
pub mod service;

use crate::rbac::{role_permissions, Permission, Role};

/// Carries the authenticated user's identity and roles after session validation.
/// Available to all route handlers that declare `user: AuthenticatedUser`.
pub struct AuthenticatedUser {
    pub user_id: i64,
    pub username: String,
    /// Raw bearer token — needed so logout can identify the session to delete.
    pub session_token: String,
    /// Roles currently assigned to this user.  Loaded fresh on every request
    /// via the session guard, so role changes take effect within one session
    /// sliding window (≤ 30 minutes).
    pub roles: Vec<Role>,
}

impl AuthenticatedUser {
    /// Returns `true` if any of the user's roles grant `perm`.
    pub fn has_permission(&self, perm: Permission) -> bool {
        self.roles
            .iter()
            .any(|&role| role_permissions(role).contains(&perm))
    }

    /// Inline permission check for use inside route handlers.
    /// Returns `Err(AppError::Forbidden)` if the check fails, allowing `?`
    /// to short-circuit the handler.
    pub fn require(&self, perm: Permission) -> crate::errors::AppResult<()> {
        if self.has_permission(perm) {
            Ok(())
        } else {
            Err(crate::errors::AppError::Forbidden(
                "Insufficient permissions".into(),
            ))
        }
    }
}
