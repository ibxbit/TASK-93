use argon2::{
    password_hash::{rand_core::OsRng, PasswordHasher, SaltString},
    Argon2,
};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use crate::rbac::{
    entity::{role, role_assignment},
    Role,
};

use super::entity::user;

// ── Default development users ─────────────────────────────────────────────────
//
// These users are seeded at startup if they do not already exist.  They are
// intended for development, testing, and initial deployment only.  In production
// replace or remove them and provision users through your identity management
// workflow.
//
// Format: (username, password, role)
const SEED_USERS: &[(&str, &str, Role)] = &[
    ("admin", "Admin123!", Role::Administrator),
    ("director", "Director123!", Role::EventDirector),
    ("referee1", "Referee123!", Role::Referee),
    ("finance1", "Finance123!", Role::FinanceClerk),
    ("auditor1", "Auditor123!", Role::Auditor),
];

/// Ensure all default users exist and have their designated role assigned.
///
/// Idempotent: users that already exist are left untouched.
/// Called once during startup, after migrations and role seeding have completed.
pub async fn seed_users(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    for &(username, password, role_variant) in SEED_USERS {
        // Skip if user already exists.
        let exists = user::Entity::find()
            .filter(user::Column::Username.eq(username))
            .one(conn)
            .await?
            .is_some();

        if exists {
            continue;
        }

        // Hash the password using Argon2id with a fresh random salt.
        let salt = SaltString::generate(&mut OsRng);
        let hash = Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| sea_orm::DbErr::Custom(format!("password hash error: {e}")))?
            .to_string();

        // Insert the user row.
        let user_model = user::ActiveModel {
            username: Set(username.to_owned()),
            password_hash: Set(hash),
            created_at: Set(Utc::now()),
            ..Default::default()
        }
        .insert(conn)
        .await?;

        // Resolve the role by name (roles must already be seeded).
        let role_model = role::Entity::find()
            .filter(role::Column::Name.eq(role_variant.as_str()))
            .one(conn)
            .await?
            .ok_or_else(|| {
                sea_orm::DbErr::Custom(format!(
                    "role '{}' not found — ensure seed_roles() runs before seed_users()",
                    role_variant.as_str()
                ))
            })?;

        // Assign the role.
        role_assignment::ActiveModel {
            user_id: Set(user_model.id),
            role_id: Set(role_model.id),
            assigned_at: Set(Utc::now()),
            assigned_by: Set(None), // system-seeded, no human assignor
            ..Default::default()
        }
        .insert(conn)
        .await?;

        tracing::info!(
            username   = username,
            role       = role_variant.as_str(),
            user_id    = user_model.id,
            "auth.user_seeded"
        );
    }

    tracing::info!("auth.seed_complete");
    Ok(())
}
