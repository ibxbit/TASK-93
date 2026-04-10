use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};

use crate::audit;
use crate::crypto::Cipher;
use crate::entity::enums::VehicleLifecycleStatus;
use crate::entity::{
    vehicle::{self as vehicle_entity, ActiveModel as VehicleActiveModel},
    vehicle_audit_log::{self as audit_entity, ActiveModel as AuditActiveModel},
};
use crate::errors::{AppError, AppResult};

use super::{
    CreateVehicleRequest, StatusTransitionRequest, UpdateVehicleRequest, VehicleAuditEntry,
    VehicleFilterQuery, VehicleResponse,
};

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_status(s: &str) -> AppResult<VehicleLifecycleStatus> {
    match s {
        "draft" => Ok(VehicleLifecycleStatus::Draft),
        "published" => Ok(VehicleLifecycleStatus::Published),
        "delisted" => Ok(VehicleLifecycleStatus::Delisted),
        "sold" => Ok(VehicleLifecycleStatus::Sold),
        _ => Err(AppError::BadRequest(format!(
            "Unknown vehicle status '{s}'. Valid values: draft, published, delisted, sold"
        ))),
    }
}

fn status_str(s: &VehicleLifecycleStatus) -> &'static str {
    match s {
        VehicleLifecycleStatus::Draft => "draft",
        VehicleLifecycleStatus::Published => "published",
        VehicleLifecycleStatus::Delisted => "delisted",
        VehicleLifecycleStatus::Sold => "sold",
    }
}

// ── Transition guard ──────────────────────────────────────────────────────────

fn is_allowed_transition(from: &VehicleLifecycleStatus, to: &VehicleLifecycleStatus) -> bool {
    use VehicleLifecycleStatus::*;
    matches!(
        (from, to),
        (Draft, Published)
            | (Published, Delisted)
            | (Published, Sold)
            | (Delisted, Published)
            | (Delisted, Sold)
    )
}

fn reason_required(to: &VehicleLifecycleStatus) -> bool {
    matches!(
        to,
        VehicleLifecycleStatus::Delisted | VehicleLifecycleStatus::Sold
    )
}

// ── VIN validation ────────────────────────────────────────────────────────────

fn validate_vin(vin: &str) -> AppResult<()> {
    if vin.len() != 17 {
        return Err(AppError::BadRequest(format!(
            "VIN must be exactly 17 characters, got {}",
            vin.len()
        )));
    }
    for ch in vin.chars() {
        if !ch.is_ascii_alphanumeric() || matches!(ch, 'I' | 'O' | 'Q') {
            return Err(AppError::BadRequest(format!(
                "VIN contains invalid character '{ch}'. \
                 Only A-H, J-N, P, R-Z, 0-9 are permitted."
            )));
        }
    }
    Ok(())
}

// ── Utility ───────────────────────────────────────────────────────────────────

fn tx_err(e: TransactionError<AppError>) -> AppError {
    match e {
        TransactionError::Transaction(e) => e,
        TransactionError::Connection(e) => AppError::Internal(e.to_string()),
    }
}

fn enc_err(e: String) -> AppError {
    AppError::Internal(format!("Encryption error: {e}"))
}

fn dec_err(e: String) -> AppError {
    AppError::Internal(format!("Decryption error: {e}"))
}

/// Build a `VehicleResponse`, decrypting `vin` and `registration_id`.
fn to_response(cipher: &Cipher, m: &vehicle_entity::Model) -> AppResult<VehicleResponse> {
    let vin_plain = cipher.decrypt(&m.vin).map_err(dec_err)?;
    let reg_plain = cipher.decrypt(&m.registration_id).map_err(dec_err)?;

    Ok(VehicleResponse {
        id: m.id,
        asset_id: m.asset_id,
        vin: vin_plain,
        registration_id: reg_plain,
        make: m.make.clone(),
        model: m.model.clone(),
        year: m.year,
        color: m.color.clone(),
        mileage: m.mileage,
        title_transfer_count: m.title_transfer_count,
        status: status_str(&m.status).to_owned(),
        status_reason: m.status_reason.clone(),
        created_by: m.created_by,
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    })
}

/// Produce an audit snapshot with sensitive identifiers redacted.
///
/// `vin` and `registration_id` are written as `[REDACTED]` so that audit
/// trails never contain PII in plaintext, even as historical snapshots.
fn snapshot_json(m: &vehicle_entity::Model) -> String {
    serde_json::json!({
        "id":                   m.id,
        "asset_id":             m.asset_id,
        "vin":                  Cipher::mask(),
        "registration_id":      Cipher::mask(),
        "make":                 m.make,
        "model":                m.model,
        "year":                 m.year,
        "color":                m.color,
        "mileage":              m.mileage,
        "title_transfer_count": m.title_transfer_count,
        "status":               status_str(&m.status),
        "status_reason":        m.status_reason,
        "updated_at":           m.updated_at.to_rfc3339(),
    })
    .to_string()
}

async fn record_audit(
    conn: &impl sea_orm::ConnectionTrait,
    vehicle_id: i64,
    changed_by: i64,
    change_type: &str,
    snapshot: &str,
) -> Result<(), sea_orm::DbErr> {
    AuditActiveModel {
        vehicle_id: Set(vehicle_id),
        changed_by: Set(changed_by),
        change_type: Set(change_type.to_owned()),
        snapshot: Set(snapshot.to_owned()),
        changed_at: Set(Utc::now()),
        ..Default::default()
    }
    .insert(conn)
    .await?;

    // Also write to the unified audit trail for cross-entity queryability.
    let snapshot_val: serde_json::Value =
        serde_json::from_str(snapshot).unwrap_or(serde_json::Value::Object(Default::default()));
    let action = format!("vehicle.{change_type}");
    audit::service::append(
        conn,
        changed_by,
        &action,
        "vehicle",
        vehicle_id,
        snapshot_val,
        serde_json::json!({}),
    )
    .await
    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Create
// ══════════════════════════════════════════════════════════════════════════════

pub async fn create_vehicle(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    user_id: i64,
    req: CreateVehicleRequest,
) -> AppResult<VehicleResponse> {
    let vin_plain = req.vin.trim().to_uppercase();
    validate_vin(&vin_plain)?;

    if req.registration_id.trim().is_empty() {
        return Err(AppError::BadRequest("registration_id is required".into()));
    }
    if req.make.trim().is_empty() {
        return Err(AppError::BadRequest("make is required".into()));
    }
    if req.model.trim().is_empty() {
        return Err(AppError::BadRequest("model is required".into()));
    }
    if req.year < 1886 || req.year > 2100 {
        return Err(AppError::BadRequest(format!(
            "year {} is out of range (1886–2100)",
            req.year
        )));
    }
    if req.mileage < 0 {
        return Err(AppError::BadRequest("mileage must be ≥ 0".into()));
    }
    if req.title_transfer_count < 0 {
        return Err(AppError::BadRequest(
            "title_transfer_count must be ≥ 0".into(),
        ));
    }

    // Encrypt identifiers and compute the VIN blind-index hash.
    let vin_encrypted = cipher.encrypt(&vin_plain).map_err(enc_err)?;
    let vin_hash = cipher.digest(&vin_plain);
    let reg_encrypted = cipher
        .encrypt(req.registration_id.trim())
        .map_err(enc_err)?;

    let now = Utc::now();

    let model = conn
        .transaction::<_, vehicle_entity::Model, AppError>(|txn| {
            let vin_encrypted = vin_encrypted.clone();
            let vin_hash = vin_hash.clone();
            let reg_encrypted = reg_encrypted.clone();
            let vin_plain = vin_plain.clone();
            let req = req.clone();
            Box::pin(async move {
                let inserted = VehicleActiveModel {
                    asset_id: Set(req.asset_id),
                    vin: Set(vin_encrypted),
                    vin_hash: Set(vin_hash),
                    registration_id: Set(reg_encrypted),
                    make: Set(req.make.trim().to_owned()),
                    model: Set(req.model.trim().to_owned()),
                    year: Set(req.year),
                    color: Set(req.color.clone()),
                    mileage: Set(req.mileage),
                    title_transfer_count: Set(req.title_transfer_count),
                    status: Set(VehicleLifecycleStatus::Draft),
                    status_reason: Set(None),
                    created_by: Set(user_id),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        AppError::Conflict(format!("VIN '{vin_plain}' already exists"))
                    } else {
                        AppError::Internal(msg)
                    }
                })?;

                let snap = snapshot_json(&inserted);
                record_audit(txn, inserted.id, user_id, "created", &snap)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(inserted)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(vehicle_id = model.id, user_id, "vehicles.created");
    to_response(cipher, &model)
}

// ══════════════════════════════════════════════════════════════════════════════
// Read
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_vehicle(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    vehicle_id: i64,
) -> AppResult<VehicleResponse> {
    let model = vehicle_entity::Entity::find_by_id(vehicle_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Vehicle {vehicle_id} not found")))?;

    to_response(cipher, &model)
}

pub async fn list_vehicles(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    filter: VehicleFilterQuery,
) -> AppResult<Vec<VehicleResponse>> {
    let mut query = vehicle_entity::Entity::find().order_by_asc(vehicle_entity::Column::Id);

    if let Some(ref st) = filter.status {
        let parsed = parse_status(st)?;
        query = query.filter(vehicle_entity::Column::Status.eq(parsed));
    }
    if let Some(ref make) = filter.make {
        query = query.filter(vehicle_entity::Column::Make.eq(make.as_str()));
    }

    let models = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    models.iter().map(|m| to_response(cipher, m)).collect()
}

// ══════════════════════════════════════════════════════════════════════════════
// Update
// ══════════════════════════════════════════════════════════════════════════════

pub async fn update_vehicle(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    vehicle_id: i64,
    user_id: i64,
    req: UpdateVehicleRequest,
) -> AppResult<VehicleResponse> {
    let existing = vehicle_entity::Entity::find_by_id(vehicle_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Vehicle {vehicle_id} not found")))?;

    if existing.status == VehicleLifecycleStatus::Sold {
        return Err(AppError::Conflict(format!(
            "Vehicle {vehicle_id} has been sold and cannot be modified"
        )));
    }

    if let Some(new_mileage) = req.mileage {
        if new_mileage < 0 {
            return Err(AppError::UnprocessableEntity("mileage must be ≥ 0".into()));
        }
        if new_mileage < existing.mileage {
            return Err(AppError::UnprocessableEntity(format!(
                "mileage cannot decrease: current={}, submitted={}",
                existing.mileage, new_mileage
            )));
        }
    }

    if let Some(year) = req.year {
        if year < 1886 || year > 2100 {
            return Err(AppError::BadRequest(format!(
                "year {} is out of range (1886–2100)",
                year
            )));
        }
    }

    // Encrypt updated registration_id before entering the transaction.
    let encrypted_reg: Option<String> = if let Some(ref reg) = req.registration_id {
        if reg.is_empty() {
            return Err(AppError::BadRequest(
                "registration_id cannot be cleared".into(),
            ));
        }
        Some(cipher.encrypt(reg.trim()).map_err(enc_err)?)
    } else {
        None
    };

    let now = Utc::now();
    let mileage_changed = req.mileage.is_some() && req.mileage.unwrap() != existing.mileage;

    let updated = conn
        .transaction::<_, vehicle_entity::Model, AppError>(|txn| {
            let req = req.clone();
            let existing = existing.clone();
            let encrypted_reg = encrypted_reg.clone();
            Box::pin(async move {
                let mut active: VehicleActiveModel = existing.into();

                if let Some(asset_id) = req.asset_id {
                    active.asset_id = Set(Some(asset_id));
                }
                if let Some(enc_reg) = encrypted_reg {
                    active.registration_id = Set(enc_reg);
                }
                if let Some(make) = req.make {
                    if make.trim().is_empty() {
                        return Err(AppError::BadRequest("make cannot be empty".into()));
                    }
                    active.make = Set(make.trim().to_owned());
                }
                if let Some(model) = req.model {
                    if model.trim().is_empty() {
                        return Err(AppError::BadRequest("model cannot be empty".into()));
                    }
                    active.model = Set(model.trim().to_owned());
                }
                if let Some(year) = req.year {
                    active.year = Set(year);
                }
                if let Some(color) = req.color {
                    active.color = Set(if color.is_empty() { None } else { Some(color) });
                }
                if let Some(mileage) = req.mileage {
                    active.mileage = Set(mileage);
                }
                active.updated_at = Set(now);

                let updated = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let snap = snapshot_json(&updated);
                let change_type = if mileage_changed {
                    "mileage_updated"
                } else {
                    "updated"
                };
                record_audit(txn, vehicle_id, user_id, change_type, &snap)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(updated)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(vehicle_id, user_id, mileage_changed, "vehicles.updated");
    to_response(cipher, &updated)
}

// ══════════════════════════════════════════════════════════════════════════════
// Status transition
// ══════════════════════════════════════════════════════════════════════════════

pub async fn transition_status(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    vehicle_id: i64,
    user_id: i64,
    req: StatusTransitionRequest,
) -> AppResult<VehicleResponse> {
    let target = parse_status(&req.status)?;

    let existing = vehicle_entity::Entity::find_by_id(vehicle_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Vehicle {vehicle_id} not found")))?;

    if existing.status == VehicleLifecycleStatus::Sold {
        return Err(AppError::Conflict(format!(
            "Vehicle {vehicle_id} is sold — no further transitions are permitted"
        )));
    }

    if !is_allowed_transition(&existing.status, &target) {
        return Err(AppError::UnprocessableEntity(format!(
            "Transition '{}' → '{}' is not permitted. \
             Allowed transitions: draft→published, published→delisted, \
             published→sold, delisted→published, delisted→sold",
            status_str(&existing.status),
            status_str(&target),
        )));
    }

    if reason_required(&target) {
        match req.reason.as_deref() {
            None | Some("") => {
                return Err(AppError::UnprocessableEntity(format!(
                    "A non-empty 'reason' is required when transitioning to '{}'",
                    status_str(&target),
                )));
            }
            _ => {}
        }
    }

    let now = Utc::now();
    let is_sale = target == VehicleLifecycleStatus::Sold;
    let current_title_count = existing.title_transfer_count;

    let updated = conn
        .transaction::<_, vehicle_entity::Model, AppError>(|txn| {
            let req = req.clone();
            let existing = existing.clone();
            let target = target.clone();
            Box::pin(async move {
                let mut active: VehicleActiveModel = existing.into();

                active.status = Set(target);
                active.status_reason = Set(req.reason.clone().filter(|r| !r.is_empty()));
                active.updated_at = Set(now);

                if is_sale {
                    active.title_transfer_count = Set(current_title_count + 1);
                }

                let updated = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let snap = snapshot_json(&updated);
                let change_type = if is_sale {
                    "title_transferred"
                } else {
                    "status_changed"
                };
                record_audit(txn, vehicle_id, user_id, change_type, &snap)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(updated)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        vehicle_id,
        user_id,
        new_status = %req.status,
        "vehicles.status_changed"
    );
    to_response(cipher, &updated)
}

// ══════════════════════════════════════════════════════════════════════════════
// Audit history
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_history(
    conn: &DatabaseConnection,
    vehicle_id: i64,
) -> AppResult<Vec<VehicleAuditEntry>> {
    vehicle_entity::Entity::find_by_id(vehicle_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Vehicle {vehicle_id} not found")))?;

    let rows = audit_entity::Entity::find()
        .filter(audit_entity::Column::VehicleId.eq(vehicle_id))
        .order_by_asc(audit_entity::Column::ChangedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(rows
        .into_iter()
        .map(|r| {
            // Snapshots already contain [REDACTED] for vin/registration_id — no
            // further masking required at read time.
            let snap: serde_json::Value =
                serde_json::from_str(&r.snapshot).unwrap_or(serde_json::Value::Null);
            VehicleAuditEntry {
                id: r.id,
                vehicle_id: r.vehicle_id,
                changed_by: r.changed_by,
                change_type: r.change_type,
                snapshot: snap,
                changed_at: r.changed_at.to_rfc3339(),
            }
        })
        .collect())
}
