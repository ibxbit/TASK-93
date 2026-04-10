use chrono::{Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};
use std::str::FromStr;

use crate::audit;
use crate::crypto::Cipher;
use crate::entity::enums::{AssetCategory, AssetStatus};
use crate::entity::{
    asset::{self as asset_entity, ActiveModel as AssetActiveModel},
    asset_audit_log::{self as audit_entity, ActiveModel as AuditActiveModel},
};
use crate::errors::{AppError, AppResult};

use super::{
    AssetFilterQuery, AssetResponse, AuditEntry, BulkImportRequest, BulkImportResponse,
    CreateAssetRequest, ImportRowError, StatusUpdateRequest, UpdateAssetRequest,
};

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_category(s: &str) -> AppResult<AssetCategory> {
    match s {
        "vehicle" => Ok(AssetCategory::Vehicle),
        "equipment" => Ok(AssetCategory::Equipment),
        "facility" => Ok(AssetCategory::Facility),
        "electronic" => Ok(AssetCategory::Electronic),
        "other" => Ok(AssetCategory::Other),
        _ => Err(AppError::BadRequest(format!(
            "Unknown category '{s}'. Valid values: vehicle, equipment, facility, \
             electronic, other"
        ))),
    }
}

fn category_str(c: &AssetCategory) -> &'static str {
    match c {
        AssetCategory::Vehicle => "vehicle",
        AssetCategory::Equipment => "equipment",
        AssetCategory::Facility => "facility",
        AssetCategory::Electronic => "electronic",
        AssetCategory::Other => "other",
    }
}

fn parse_status(s: &str) -> AppResult<AssetStatus> {
    match s {
        "in_service" => Ok(AssetStatus::InService),
        "out_for_repair" => Ok(AssetStatus::OutForRepair),
        "retired" => Ok(AssetStatus::Retired),
        _ => Err(AppError::BadRequest(format!(
            "Unknown status '{s}'. Valid values: in_service, out_for_repair, retired"
        ))),
    }
}

fn status_str(s: &AssetStatus) -> &'static str {
    match s {
        AssetStatus::InService => "in_service",
        AssetStatus::OutForRepair => "out_for_repair",
        AssetStatus::Retired => "retired",
    }
}

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").map_err(|_| {
        AppError::BadRequest(format!(
            "Invalid procurement_date '{s}'; expected YYYY-MM-DD"
        ))
    })
}

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

// ── Depreciation ──────────────────────────────────────────────────────────────

fn compute_depreciation(
    procurement_cost: Option<Decimal>,
    procurement_date: Option<&str>,
    useful_life_months: Option<i32>,
) -> (Option<f64>, Option<f64>, bool) {
    let (Some(cost), Some(date_str), Some(life)) =
        (procurement_cost, procurement_date, useful_life_months)
    else {
        return (None, None, false);
    };

    if life <= 0 {
        return (None, None, false);
    }

    let cost_f = match cost.to_string().parse::<f64>() {
        Ok(v) => v,
        Err(_) => return (None, None, false),
    };

    let monthly = cost_f / life as f64;

    let Ok(start_date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") else {
        return (None, None, false);
    };

    let today = Utc::now().date_naive();
    let months_elapsed = (today.year() - start_date.year()) * 12
        + (today.month() as i32 - start_date.month() as i32);
    let months_elapsed = months_elapsed.max(0) as i32;

    let accumulated = (monthly * months_elapsed as f64).min(cost_f);
    let book_value = (cost_f - accumulated).max(0.0);
    let fully_depreciated = months_elapsed >= life;

    (Some(monthly), Some(book_value), fully_depreciated)
}

// ── Model → response ──────────────────────────────────────────────────────────

/// Build an `AssetResponse`, decrypting `serial_number` if present.
fn to_response(cipher: &Cipher, m: &asset_entity::Model) -> AppResult<AssetResponse> {
    let (monthly, book_value, fully_dep) = compute_depreciation(
        m.procurement_cost,
        m.procurement_date.as_deref(),
        m.useful_life_months,
    );
    let cost_f = m
        .procurement_cost
        .map(|d| d.to_string().parse::<f64>().unwrap_or(0.0));

    let serial_number_plain = cipher
        .decrypt_opt(m.serial_number.as_deref())
        .map_err(dec_err)?;

    Ok(AssetResponse {
        id: m.id,
        asset_code: m.asset_code.clone(),
        category: category_str(&m.category).to_owned(),
        brand: m.brand.clone(),
        model: m.model.clone(),
        serial_number: serial_number_plain,
        status: status_str(&m.status).to_owned(),
        owner_id: m.owner_id,
        responsible_person_id: m.responsible_person_id,
        procurement_cost: cost_f,
        procurement_date: m.procurement_date.clone(),
        useful_life_months: m.useful_life_months,
        monthly_depreciation: monthly,
        current_book_value: book_value,
        fully_depreciated: fully_dep,
        notes: m.notes.clone(),
        created_at: m.created_at.to_rfc3339(),
        updated_at: m.updated_at.to_rfc3339(),
    })
}

/// Produce an audit snapshot with `serial_number` redacted.
///
/// The snapshot is written to the append-only audit log — sensitive identifiers
/// must never appear in plaintext in audit trails.
fn snapshot_json(m: &asset_entity::Model) -> String {
    serde_json::json!({
        "id":                    m.id,
        "asset_code":            m.asset_code,
        "category":              category_str(&m.category),
        "brand":                 m.brand,
        "model":                 m.model,
        "serial_number":         Cipher::mask(),
        "status":                status_str(&m.status),
        "owner_id":              m.owner_id,
        "responsible_person_id": m.responsible_person_id,
        "procurement_cost":      m.procurement_cost.map(|d| d.to_string()),
        "procurement_date":      m.procurement_date,
        "useful_life_months":    m.useful_life_months,
        "notes":                 m.notes,
        "updated_at":            m.updated_at.to_rfc3339(),
    })
    .to_string()
}

async fn record_audit(
    conn: &impl sea_orm::ConnectionTrait,
    asset_id: i64,
    changed_by: i64,
    change_type: &str,
    snapshot: &str,
) -> Result<(), sea_orm::DbErr> {
    AuditActiveModel {
        asset_id: Set(asset_id),
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
    let action = format!("asset.{change_type}");
    audit::service::append(
        conn,
        changed_by,
        &action,
        "asset",
        asset_id,
        snapshot_val,
        serde_json::json!({}),
    )
    .await
    .map_err(|e| sea_orm::DbErr::Custom(e.to_string()))?;

    Ok(())
}

// ── Validation ────────────────────────────────────────────────────────────────

fn validate_create_fields(req: &CreateAssetRequest) -> AppResult<()> {
    if req.asset_code.trim().is_empty() {
        return Err(AppError::BadRequest("asset_code is required".into()));
    }
    if req.brand.trim().is_empty() {
        return Err(AppError::BadRequest("brand is required".into()));
    }
    if req.model.trim().is_empty() {
        return Err(AppError::BadRequest("model is required".into()));
    }
    if let Some(ref date) = req.procurement_date {
        parse_date(date)?;
    }
    if let Some(life) = req.useful_life_months {
        if life <= 0 {
            return Err(AppError::BadRequest(
                "useful_life_months must be > 0".into(),
            ));
        }
    }
    if let Some(cost) = req.procurement_cost {
        if cost < 0.0 || !cost.is_finite() {
            return Err(AppError::BadRequest(
                "procurement_cost must be a non-negative finite number".into(),
            ));
        }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Create
// ══════════════════════════════════════════════════════════════════════════════

pub async fn create_asset(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    user_id: i64,
    req: CreateAssetRequest,
) -> AppResult<AssetResponse> {
    validate_create_fields(&req)?;
    let category = parse_category(&req.category)?;
    let status = parse_status(req.status.as_deref().unwrap_or("in_service"))?;
    let cost = req
        .procurement_cost
        .map(|c| Decimal::from_str(&c.to_string()))
        .transpose()
        .map_err(|_| AppError::BadRequest("Invalid procurement_cost".into()))?;

    // Encrypt serial_number before storing.
    let serial_encrypted = cipher
        .encrypt_opt(req.serial_number.as_deref())
        .map_err(enc_err)?;

    let now = Utc::now();

    let model = conn
        .transaction::<_, asset_entity::Model, AppError>(|txn| {
            let req = req.clone();
            let category = category.clone();
            let status = status.clone();
            let serial_encrypted = serial_encrypted.clone();
            Box::pin(async move {
                let inserted = AssetActiveModel {
                    asset_code: Set(req.asset_code.trim().to_owned()),
                    category: Set(category),
                    brand: Set(req.brand.trim().to_owned()),
                    model: Set(req.model.trim().to_owned()),
                    serial_number: Set(serial_encrypted),
                    status: Set(status),
                    owner_id: Set(req.owner_id),
                    responsible_person_id: Set(req.responsible_person_id),
                    procurement_cost: Set(cost),
                    procurement_date: Set(req.procurement_date.clone()),
                    useful_life_months: Set(req.useful_life_months),
                    notes: Set(req.notes.clone()),
                    created_at: Set(now),
                    updated_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        AppError::Conflict(format!(
                            "Asset code '{}' already exists",
                            req.asset_code
                        ))
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

    tracing::info!(
        asset_id = model.id,
        asset_code = %model.asset_code,
        user_id,
        "assets.created"
    );
    to_response(cipher, &model)
}

// ══════════════════════════════════════════════════════════════════════════════
// Read
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_asset(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    asset_id: i64,
) -> AppResult<AssetResponse> {
    let model = asset_entity::Entity::find_by_id(asset_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} not found")))?;

    to_response(cipher, &model)
}

pub async fn list_assets(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    filter: AssetFilterQuery,
) -> AppResult<Vec<AssetResponse>> {
    let mut query = asset_entity::Entity::find().order_by_asc(asset_entity::Column::AssetCode);

    if let Some(ref cat) = filter.category {
        let parsed = parse_category(cat)?;
        query = query.filter(asset_entity::Column::Category.eq(parsed));
    }
    if let Some(ref st) = filter.status {
        let parsed = parse_status(st)?;
        query = query.filter(asset_entity::Column::Status.eq(parsed));
    }

    let models = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    models.iter().map(|m| to_response(cipher, m)).collect()
}

/// Export all assets with decrypted `serial_number`.
///
/// Note: this is a management data export, not an audit log export.
/// The live `serial_number` value is returned in plaintext for authorised
/// callers.  Audit log snapshots use `[REDACTED]` regardless.
pub async fn export_assets(
    conn: &DatabaseConnection,
    cipher: &Cipher,
) -> AppResult<Vec<AssetResponse>> {
    let models = asset_entity::Entity::find()
        .order_by_asc(asset_entity::Column::AssetCode)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    models.iter().map(|m| to_response(cipher, m)).collect()
}

// ══════════════════════════════════════════════════════════════════════════════
// Update
// ══════════════════════════════════════════════════════════════════════════════

pub async fn update_asset(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    asset_id: i64,
    user_id: i64,
    req: UpdateAssetRequest,
) -> AppResult<AssetResponse> {
    if let Some(ref date) = req.procurement_date {
        parse_date(date)?;
    }
    if let Some(life) = req.useful_life_months {
        if life <= 0 {
            return Err(AppError::BadRequest(
                "useful_life_months must be > 0".into(),
            ));
        }
    }
    if let Some(cost) = req.procurement_cost {
        if cost < 0.0 || !cost.is_finite() {
            return Err(AppError::BadRequest(
                "procurement_cost must be a non-negative finite number".into(),
            ));
        }
    }

    let existing = asset_entity::Entity::find_by_id(asset_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} not found")))?;

    // Encrypt updated serial_number before entering the transaction.
    let new_serial_encrypted: Option<Option<String>> = if let Some(ref sn) = req.serial_number {
        if sn.is_empty() {
            Some(None) // clear the field
        } else {
            Some(Some(cipher.encrypt(sn).map_err(enc_err)?))
        }
    } else {
        None // field not supplied — leave unchanged
    };

    let now = Utc::now();

    let updated = conn
        .transaction::<_, asset_entity::Model, AppError>(|txn| {
            let req = req.clone();
            let existing = existing.clone();
            let new_serial_encrypted = new_serial_encrypted.clone();
            Box::pin(async move {
                let mut active: AssetActiveModel = existing.into();

                if let Some(cat) = req.category {
                    active.category = Set(parse_category(&cat)?);
                }
                if let Some(brand) = req.brand {
                    active.brand = Set(brand);
                }
                if let Some(model) = req.model {
                    active.model = Set(model);
                }
                if let Some(enc_sn) = new_serial_encrypted {
                    active.serial_number = Set(enc_sn);
                }
                if let Some(oid) = req.owner_id {
                    active.owner_id = Set(Some(oid));
                }
                if let Some(rid) = req.responsible_person_id {
                    active.responsible_person_id = Set(Some(rid));
                }
                if let Some(cost) = req.procurement_cost {
                    let dec = Decimal::from_str(&cost.to_string())
                        .map_err(|_| AppError::BadRequest("Invalid procurement_cost".into()))?;
                    active.procurement_cost = Set(Some(dec));
                }
                if let Some(date) = req.procurement_date {
                    active.procurement_date = Set(if date.is_empty() { None } else { Some(date) });
                }
                if let Some(life) = req.useful_life_months {
                    active.useful_life_months = Set(Some(life));
                }
                if let Some(notes) = req.notes {
                    active.notes = Set(if notes.is_empty() { None } else { Some(notes) });
                }
                active.updated_at = Set(now);

                let updated = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let snap = snapshot_json(&updated);
                record_audit(txn, asset_id, user_id, "updated", &snap)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(updated)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(asset_id, user_id, "assets.updated");
    to_response(cipher, &updated)
}

pub async fn update_status(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    asset_id: i64,
    user_id: i64,
    req: StatusUpdateRequest,
) -> AppResult<AssetResponse> {
    let new_status = parse_status(&req.status)?;

    let existing = asset_entity::Entity::find_by_id(asset_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} not found")))?;

    if existing.status == AssetStatus::Retired && new_status != AssetStatus::Retired {
        return Err(AppError::Conflict(format!(
            "Asset {asset_id} is retired and cannot transition to '{}'",
            status_str(&new_status)
        )));
    }

    let now = Utc::now();

    let updated = conn
        .transaction::<_, asset_entity::Model, AppError>(|txn| {
            let existing = existing.clone();
            let new_status = new_status.clone();
            Box::pin(async move {
                let mut active: AssetActiveModel = existing.into();
                active.status = Set(new_status);
                active.updated_at = Set(now);

                let updated = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let snap = snapshot_json(&updated);
                record_audit(txn, asset_id, user_id, "status_changed", &snap)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                Ok(updated)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        asset_id,
        user_id,
        new_status = %req.status,
        "assets.status_changed"
    );
    to_response(cipher, &updated)
}

// ══════════════════════════════════════════════════════════════════════════════
// Bulk import
// ══════════════════════════════════════════════════════════════════════════════

pub async fn import_assets(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    user_id: i64,
    req: BulkImportRequest,
) -> AppResult<BulkImportResponse> {
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut errors: Vec<ImportRowError> = Vec::new();

    for (idx, row) in req.assets.into_iter().enumerate() {
        if let Err(e) = validate_create_fields(&row) {
            errors.push(ImportRowError {
                index: idx,
                asset_code: Some(row.asset_code.clone()),
                reason: e.to_string(),
            });
            continue;
        }

        let category = match parse_category(&row.category) {
            Ok(c) => c,
            Err(e) => {
                errors.push(ImportRowError {
                    index: idx,
                    asset_code: Some(row.asset_code.clone()),
                    reason: e.to_string(),
                });
                continue;
            }
        };
        let status = match parse_status(row.status.as_deref().unwrap_or("in_service")) {
            Ok(s) => s,
            Err(e) => {
                errors.push(ImportRowError {
                    index: idx,
                    asset_code: Some(row.asset_code.clone()),
                    reason: e.to_string(),
                });
                continue;
            }
        };

        // Deduplication by (asset_code, serial_number).
        let existing = asset_entity::Entity::find()
            .filter(asset_entity::Column::AssetCode.eq(row.asset_code.trim()))
            .one(conn)
            .await
            .map_err(|e| AppError::Internal(e.to_string()))?;

        if let Some(ref ex) = existing {
            // Decrypt the stored serial_number to compare with the plaintext input.
            let existing_sn = match cipher.decrypt_opt(ex.serial_number.as_deref()) {
                Ok(v) => v,
                Err(e) => {
                    errors.push(ImportRowError {
                        index: idx,
                        asset_code: Some(row.asset_code.clone()),
                        reason: format!("Decryption error during dedup check: {e}"),
                    });
                    continue;
                }
            };
            let serials_match = existing_sn.as_deref() == row.serial_number.as_deref();
            if serials_match {
                skipped += 1;
                continue;
            } else {
                errors.push(ImportRowError {
                    index: idx,
                    asset_code: Some(row.asset_code.clone()),
                    reason: format!(
                        "asset_code '{}' already exists with a different serial_number",
                        row.asset_code
                    ),
                });
                continue;
            }
        }

        let serial_encrypted = match cipher.encrypt_opt(row.serial_number.as_deref()) {
            Ok(v) => v,
            Err(e) => {
                errors.push(ImportRowError {
                    index: idx,
                    asset_code: Some(row.asset_code.clone()),
                    reason: format!("Encryption error: {e}"),
                });
                continue;
            }
        };

        let cost = row
            .procurement_cost
            .map(|c| Decimal::from_str(&c.to_string()))
            .transpose()
            .map_err(|_| AppError::BadRequest("Invalid procurement_cost".into()))?;

        let now = Utc::now();

        let result = conn
            .transaction::<_, asset_entity::Model, AppError>(|txn| {
                let row = row.clone();
                let category = category.clone();
                let status = status.clone();
                let serial_encrypted = serial_encrypted.clone();
                Box::pin(async move {
                    let inserted = AssetActiveModel {
                        asset_code: Set(row.asset_code.trim().to_owned()),
                        category: Set(category),
                        brand: Set(row.brand.trim().to_owned()),
                        model: Set(row.model.trim().to_owned()),
                        serial_number: Set(serial_encrypted),
                        status: Set(status),
                        owner_id: Set(row.owner_id),
                        responsible_person_id: Set(row.responsible_person_id),
                        procurement_cost: Set(cost),
                        procurement_date: Set(row.procurement_date.clone()),
                        useful_life_months: Set(row.useful_life_months),
                        notes: Set(row.notes.clone()),
                        created_at: Set(now),
                        updated_at: Set(now),
                        ..Default::default()
                    }
                    .insert(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                    let snap = snapshot_json(&inserted);
                    record_audit(txn, inserted.id, user_id, "imported", &snap)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?;

                    Ok(inserted)
                })
            })
            .await;

        match result {
            Ok(_) => imported += 1,
            Err(e) => {
                let msg = match e {
                    TransactionError::Transaction(ae) => ae.to_string(),
                    TransactionError::Connection(de) => de.to_string(),
                };
                errors.push(ImportRowError {
                    index: idx,
                    asset_code: Some(row.asset_code.clone()),
                    reason: msg,
                });
            }
        }
    }

    tracing::info!(
        user_id,
        imported,
        skipped,
        error_count = errors.len(),
        "assets.bulk_imported"
    );
    Ok(BulkImportResponse {
        imported,
        skipped,
        errors,
    })
}

// ══════════════════════════════════════════════════════════════════════════════
// Audit history
// ══════════════════════════════════════════════════════════════════════════════

pub async fn get_history(conn: &DatabaseConnection, asset_id: i64) -> AppResult<Vec<AuditEntry>> {
    asset_entity::Entity::find_by_id(asset_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Asset {asset_id} not found")))?;

    let rows = audit_entity::Entity::find()
        .filter(audit_entity::Column::AssetId.eq(asset_id))
        .order_by_asc(audit_entity::Column::ChangedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Snapshots already contain [REDACTED] for serial_number — no further
    // masking required at read time.
    let entries = rows
        .into_iter()
        .map(|r| {
            let snap: serde_json::Value =
                serde_json::from_str(&r.snapshot).unwrap_or(serde_json::Value::Null);
            AuditEntry {
                id: r.id,
                asset_id: r.asset_id,
                changed_by: r.changed_by,
                change_type: r.change_type,
                snapshot: snap,
                changed_at: r.changed_at.to_rfc3339(),
            }
        })
        .collect();

    Ok(entries)
}
