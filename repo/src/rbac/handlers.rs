use chrono::Utc;
use rocket::{serde::json::Json, Request, State};
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::errors::{AppError, AppResult, ErrorBody};
use crate::middleware::correlation::CorrelationId;
use crate::rbac::{
    entity::{role, role_assignment},
    guards::RequireRolesManage,
    Role,
};

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RoleChangeRequest {
    pub user_id: i64,
    pub role: Role,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Serialize)]
pub struct UserRolesResponse {
    pub user_id: i64,
    pub roles: Vec<String>,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn resolve_role_id(conn: &DatabaseConnection, role: Role) -> AppResult<i64> {
    role::Entity::find()
        .filter(role::Column::Name.eq(role.as_str()))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .map(|m| m.id)
        .ok_or_else(|| AppError::Internal(format!("role '{}' is not seeded", role.as_str())))
}

// ── Route handlers ────────────────────────────────────────────────────────────

/// Assign a role to a user.  Idempotency is enforced: re-assigning an already
/// held role returns HTTP 409 Conflict rather than creating a duplicate row.
#[post("/admin/roles/assign", data = "<body>")]
pub async fn assign_role(
    admin: RequireRolesManage,
    body: Json<RoleChangeRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<OkResponse>> {
    let role_id = resolve_role_id(conn.inner(), body.role).await?;

    let already_assigned = role_assignment::Entity::find()
        .filter(role_assignment::Column::UserId.eq(body.user_id))
        .filter(role_assignment::Column::RoleId.eq(role_id))
        .one(conn.inner())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .is_some();

    if already_assigned {
        return Err(AppError::Conflict(format!(
            "user {} already holds role '{}'",
            body.user_id,
            body.role.as_str()
        )));
    }

    role_assignment::ActiveModel {
        user_id: Set(body.user_id),
        role_id: Set(role_id),
        assigned_at: Set(Utc::now()),
        assigned_by: Set(Some(admin.0.user_id)),
        ..Default::default()
    }
    .insert(conn.inner())
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        admin_user_id = admin.0.user_id,
        target_user_id = body.user_id,
        role = body.role.as_str(),
        "rbac.role_assigned",
    );

    Ok(Json(OkResponse { ok: true }))
}

/// Revoke a role from a user.  Returns HTTP 404 if the assignment does not exist.
#[post("/admin/roles/revoke", data = "<body>")]
pub async fn revoke_role(
    admin: RequireRolesManage,
    body: Json<RoleChangeRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<OkResponse>> {
    let role_id = resolve_role_id(conn.inner(), body.role).await?;

    let deleted = role_assignment::Entity::delete_many()
        .filter(role_assignment::Column::UserId.eq(body.user_id))
        .filter(role_assignment::Column::RoleId.eq(role_id))
        .exec(conn.inner())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    if deleted.rows_affected == 0 {
        return Err(AppError::NotFound(format!(
            "user {} does not hold role '{}'",
            body.user_id,
            body.role.as_str()
        )));
    }

    tracing::info!(
        admin_user_id = admin.0.user_id,
        target_user_id = body.user_id,
        role = body.role.as_str(),
        "rbac.role_revoked",
    );

    Ok(Json(OkResponse { ok: true }))
}

/// List all roles currently assigned to a user.
#[get("/admin/users/<user_id>/roles")]
pub async fn list_user_roles(
    _guard: RequireRolesManage,
    user_id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<UserRolesResponse>> {
    let assignments = role_assignment::Entity::find()
        .filter(role_assignment::Column::UserId.eq(user_id))
        .all(conn.inner())
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let role_ids: Vec<i64> = assignments.iter().map(|a| a.role_id).collect();

    let role_names = if role_ids.is_empty() {
        vec![]
    } else {
        role::Entity::find()
            .filter(role::Column::Id.is_in(role_ids))
            .all(conn.inner())
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?
            .into_iter()
            .map(|r| r.name)
            .collect()
    };

    Ok(Json(UserRolesResponse {
        user_id,
        roles: role_names,
    }))
}

// ── Catchers ──────────────────────────────────────────────────────────────────

#[catch(403)]
pub fn catch_forbidden(req: &Request) -> Json<ErrorBody> {
    let cid = req.local_cache(|| CorrelationId(String::new())).0.clone();
    Json(ErrorBody {
        code: "FORBIDDEN",
        message: "Insufficient permissions for this operation".to_string(),
        correlation_id: if cid.is_empty() { None } else { Some(cid) },
    })
}
