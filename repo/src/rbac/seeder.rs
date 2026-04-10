use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};

use super::{entity::role, Role};

/// Ensures all five canonical roles exist in the `roles` table.
///
/// This is idempotent: roles that already exist are left untouched.
/// Called once during startup, after migrations have run.
pub async fn seed_roles(conn: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    for variant in Role::all() {
        let exists = role::Entity::find()
            .filter(role::Column::Name.eq(variant.as_str()))
            .one(conn)
            .await?
            .is_some();

        if !exists {
            role::ActiveModel {
                name: Set(variant.as_str().to_owned()),
                description: Set(variant.description().to_owned()),
                created_at: Set(Utc::now()),
                ..Default::default()
            }
            .insert(conn)
            .await?;

            tracing::info!(role = variant.as_str(), "rbac.role_seeded");
        }
    }

    tracing::info!("rbac.seed_complete");
    Ok(())
}
