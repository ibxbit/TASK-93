use chrono::{DateTime, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};

use crate::audit;
use crate::auth::AuthenticatedUser;
use crate::crypto::Cipher;
use crate::entity::enums::{
    ExceptionType, InvoiceStatus, PaymentEntryStatus, PaymentMethod, RefundStatus,
};
use crate::entity::{
    invoice::{self as invoice_entity, ActiveModel as InvoiceActiveModel},
    invoice_line::{self as line_entity, ActiveModel as LineActiveModel},
    payment_entry::{self as payment_entity, ActiveModel as PaymentActiveModel},
    payment_exception::{self as exception_entity, ActiveModel as ExceptionActiveModel},
    payment_refund::{self as refund_entity, ActiveModel as RefundActiveModel},
};
use crate::errors::{AppError, AppResult};
use crate::rbac::Permission;

use super::{
    ApproveRefundRequest, ExceptionResponse, HandleExceptionRequest, PaymentResponse,
    RecordPaymentRequest, RefundResponse, RejectRefundRequest, RequestRefundRequest,
};

// ── Constants ─────────────────────────────────────────────────────────────────

const AUDITOR_THRESHOLD: Decimal = Decimal::from_parts(1000, 0, 0, false, 0);
const MONEY_DP: u32 = 4;

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_method(s: &str) -> AppResult<PaymentMethod> {
    match s {
        "cash" => Ok(PaymentMethod::Cash),
        "cheque" | "check" => Ok(PaymentMethod::Cheque),
        "ach" => Ok(PaymentMethod::Ach),
        "bank_transfer" => Ok(PaymentMethod::BankTransfer),
        "card" => Ok(PaymentMethod::Card),
        _ => Err(AppError::BadRequest(format!(
            "Unknown payment method '{s}'. Valid: cash, cheque, ach, bank_transfer, card"
        ))),
    }
}

fn method_str(m: &PaymentMethod) -> &'static str {
    match m {
        PaymentMethod::Cash => "cash",
        PaymentMethod::Cheque => "cheque",
        PaymentMethod::Ach => "ach",
        PaymentMethod::BankTransfer => "bank_transfer",
        PaymentMethod::Card => "card",
    }
}

fn status_str(s: &PaymentEntryStatus) -> &'static str {
    match s {
        PaymentEntryStatus::Active => "active",
        PaymentEntryStatus::Voided => "voided",
        PaymentEntryStatus::Reversed => "reversed",
        PaymentEntryStatus::Disputed => "disputed",
    }
}

fn refund_status_str(s: &RefundStatus) -> &'static str {
    match s {
        RefundStatus::PendingFinance => "pending_finance",
        RefundStatus::PendingAuditor => "pending_auditor",
        RefundStatus::Approved => "approved",
        RefundStatus::Rejected => "rejected",
    }
}

fn parse_exception_type(s: &str) -> AppResult<ExceptionType> {
    match s {
        "void" => Ok(ExceptionType::Void),
        "reversal" => Ok(ExceptionType::Reversal),
        "dispute" => Ok(ExceptionType::Dispute),
        _ => Err(AppError::BadRequest(format!(
            "Unknown exception_type '{s}'. Valid: void, reversal, dispute"
        ))),
    }
}

fn exception_type_str(e: &ExceptionType) -> &'static str {
    match e {
        ExceptionType::Void => "void",
        ExceptionType::Reversal => "reversal",
        ExceptionType::Dispute => "dispute",
    }
}

fn dec(v: f64) -> AppResult<Decimal> {
    Decimal::from_f64(v).ok_or_else(|| AppError::BadRequest(format!("Invalid numeric value: {v}")))
}

fn tx_err(e: TransactionError<AppError>) -> AppError {
    match e {
        TransactionError::Transaction(e) => e,
        TransactionError::Connection(e) => {
            eprintln!("[INTERNAL_ERROR] DB connection error: {}", e);
            AppError::Internal(e.to_string())
        },
    }
}

fn parse_datetime(s: &str) -> AppResult<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .map(|dt| dt.with_timezone(&Utc))
        .map_err(|_| {
            AppError::BadRequest(format!(
                "Invalid received_at '{s}'; expected ISO 8601, e.g. 2024-07-01T14:30:00Z"
            ))
        })
}

// ── Crypto helpers ────────────────────────────────────────────────────────────

fn enc_err(e: String) -> AppError {
    eprintln!("[INTERNAL_ERROR] Encryption error: {}", e);
    AppError::Internal(format!("Encryption error: {e}"))
}

fn dec_err(e: String) -> AppError {
    eprintln!("[INTERNAL_ERROR] Decryption error: {}", e);
    AppError::Internal(format!("Decryption error: {e}"))
}

// ── Model → response ──────────────────────────────────────────────────────────

fn exception_to_response(e: &exception_entity::Model) -> ExceptionResponse {
    ExceptionResponse {
        id: e.id,
        payment_id: e.payment_id,
        exception_type: exception_type_str(&e.exception_type).to_owned(),
        reason: e.reason.clone(),
        raised_by: e.raised_by,
        created_at: e.created_at.to_rfc3339(),
    }
}

fn refund_to_response(r: &refund_entity::Model) -> RefundResponse {
    RefundResponse {
        id: r.id,
        payment_id: r.payment_id,
        invoice_line_id: r.invoice_line_id,
        amount: r.amount.to_string(),
        reason: r.reason.clone(),
        status: refund_status_str(&r.status).to_owned(),
        requested_by: r.requested_by,
        finance_approved_by: r.finance_approved_by,
        auditor_approved_by: r.auditor_approved_by,
        rejected_by: r.rejected_by,
        rejection_reason: r.rejection_reason.clone(),
        created_at: r.created_at.to_rfc3339(),
        updated_at: r.updated_at.to_rfc3339(),
    }
}

/// Build a full `PaymentResponse` from a DB row, decrypting `external_reference`.
async fn load_payment_details(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    payment: payment_entity::Model,
) -> AppResult<PaymentResponse> {
    let exceptions = exception_entity::Entity::find()
        .filter(exception_entity::Column::PaymentId.eq(payment.id))
        .order_by_asc(exception_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| {
            eprintln!("[INTERNAL_ERROR] DB error loading exceptions: {}", e);
            AppError::Internal(e.to_string())
        })?;

    let refunds = refund_entity::Entity::find()
        .filter(refund_entity::Column::PaymentId.eq(payment.id))
        .order_by_asc(refund_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| {
            eprintln!("[INTERNAL_ERROR] DB error loading refunds: {}", e);
            AppError::Internal(e.to_string())
        })?;

    // Decrypt the external reference for the API response.
    let ext_ref_plain = cipher
        .decrypt(&payment.external_reference)
        .map_err(|e| {
            eprintln!("[INTERNAL_ERROR] Decryption error: {}", e);
            dec_err(e)
        })?;

    Ok(PaymentResponse {
        id: payment.id,
        invoice_id: payment.invoice_id,
        method: method_str(&payment.method).to_owned(),
        amount: payment.amount.to_string(),
        external_reference: ext_ref_plain,
        received_at: payment.received_at.to_rfc3339(),
        notes: payment.notes,
        status: status_str(&payment.status).to_owned(),
        recorded_by: payment.recorded_by,
        created_at: payment.created_at.to_rfc3339(),
        exceptions: exceptions.iter().map(exception_to_response).collect(),
        refunds: refunds.iter().map(refund_to_response).collect(),
    })
}

// ── Effective paid total ──────────────────────────────────────────────────────

async fn effective_paid_total(
    conn: &impl sea_orm::ConnectionTrait,
    invoice_id: i64,
) -> Result<Decimal, sea_orm::DbErr> {
    let payments = payment_entity::Entity::find()
        .filter(payment_entity::Column::InvoiceId.eq(invoice_id))
        .filter(payment_entity::Column::Status.eq(PaymentEntryStatus::Active))
        .all(conn)
        .await?;

    Ok(payments
        .iter()
        .fold(Decimal::ZERO, |acc, p| acc + p.amount)
        .round_dp(MONEY_DP))
}

// ══════════════════════════════════════════════════════════════════════════════
// Record payment
// ══════════════════════════════════════════════════════════════════════════════

pub async fn record_payment(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    invoice_id: i64,
    user_id: i64,
    req: RecordPaymentRequest,
) -> AppResult<PaymentResponse> {
    // ── Validate inputs ───────────────────────────────────────────────────────

    if req.external_reference.trim().is_empty() {
        return Err(AppError::BadRequest(
            "external_reference is required".into(),
        ));
    }
    if req.amount <= 0.0 || !req.amount.is_finite() {
        return Err(AppError::BadRequest(
            "amount must be a positive finite number".into(),
        ));
    }

    let method = parse_method(&req.method)?;
    let amount = dec(req.amount)?;
    let received_at = parse_datetime(&req.received_at)?;
    let ext_ref_plain = req.external_reference.trim().to_owned();

    // ── Encrypt the reference and compute its blind-index hash ────────────────

    let ext_ref_encrypted = cipher.encrypt(&ext_ref_plain).map_err(enc_err)?;
    let ref_hash = cipher.digest(&ext_ref_plain);

    // ── Load invoice ──────────────────────────────────────────────────────────

    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    if !matches!(inv.status, InvoiceStatus::Issued | InvoiceStatus::Overdue) {
        return Err(AppError::Conflict(format!(
            "Invoice {invoice_id} is '{}' — payments can only be recorded against issued \
             or overdue invoices",
            match inv.status {
                InvoiceStatus::Draft => "draft",
                InvoiceStatus::Paid => "paid",
                InvoiceStatus::Cancelled => "cancelled",
                _ => "unknown",
            }
        )));
    }

    // ── Idempotency: look up by blind-index hash ──────────────────────────────

    if let Some(existing) = payment_entity::Entity::find()
        .filter(payment_entity::Column::ReferenceHash.eq(&ref_hash))
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
    {
        if existing.invoice_id != invoice_id {
            return Err(AppError::Conflict(format!(
                "external_reference already recorded against a different invoice ({})",
                existing.invoice_id
            )));
        }
        tracing::info!(
            payment_id = existing.id,
            invoice_id,
            "billing.payment_duplicate_idempotent"
        );
        return load_payment_details(conn, cipher, existing).await;
    }

    // ── Insert payment + maybe mark invoice paid ───────────────────────────────

    let now = Utc::now();

    let result = conn
        .transaction::<_, (invoice_entity::Model, i64), AppError>(|txn| {
            let ext_ref_encrypted = ext_ref_encrypted.clone();
            let ref_hash = ref_hash.clone();
            let inv = inv.clone();
            let method = method.clone();
            let req = req.clone();
            Box::pin(async move {
                let payment = PaymentActiveModel {
                    invoice_id: Set(invoice_id),
                    method: Set(method),
                    amount: Set(amount),
                    received_at: Set(received_at),
                    external_reference: Set(ext_ref_encrypted),
                    reference_hash: Set(ref_hash),
                    recorded_by: Set(user_id),
                    notes: Set(req.notes.clone()),
                    status: Set(PaymentEntryStatus::Active),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| {
                    let msg = e.to_string();
                    if msg.contains("UNIQUE") {
                        AppError::Conflict("external_reference already exists".into())
                    } else {
                        AppError::Internal(msg)
                    }
                })?;

                let payment_id = payment.id;

                let paid_total = effective_paid_total(txn, invoice_id)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let updated_inv = if paid_total >= inv.total && inv.total > Decimal::ZERO {
                    let mut active: InvoiceActiveModel = inv.into();
                    active.status = Set(InvoiceStatus::Paid);
                    active.updated_at = Set(now);
                    active
                        .update(txn)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?
                } else {
                    inv
                };

                Ok((updated_inv, payment_id))
            })
        })
        .await;

    let (updated_inv, new_payment_id) = match result {
        Ok(val) => val,
        Err(e) => {
            let is_conflict = match &e {
                TransactionError::Transaction(AppError::Conflict(_)) => true,
                _ => e.to_string().contains("already exists") || e.to_string().contains("UNIQUE"),
            };

            if is_conflict {
                // Secondary check: if it was a race condition or stale data from a previous run,
                // the record should now be visible or already present in the DB.
                if let Some(existing) = payment_entity::Entity::find()
                    .filter(payment_entity::Column::ReferenceHash.eq(&ref_hash))
                    .one(conn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?
                {
                    if existing.invoice_id != invoice_id {
                        return Err(AppError::Conflict(format!(
                            "external_reference already recorded against a different invoice ({})",
                            existing.invoice_id
                        )));
                    }
                    // Success: return the existing record (idempotent result)
                    return load_payment_details(conn, cipher, existing).await;
                }
            }
            return Err(tx_err(e));
        }
    };

    tracing::info!(
        invoice_id,
        user_id,
        amount = %amount,
        invoice_status = ?updated_inv.status,
        "billing.payment_recorded"
    );

    let payment = payment_entity::Entity::find_by_id(new_payment_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Failed to reload payment after insert".into()))?;

    let resp = load_payment_details(conn, cipher, payment).await?;
    let audit_snapshot = {
        let mut s = serde_json::to_value(&resp).unwrap_or_default();
        if let Some(obj) = s.as_object_mut() {
            obj.insert(
                "external_reference".into(),
                serde_json::Value::String(Cipher::mask().to_owned()),
            );
        }
        s
    };
    audit::service::append(
        conn,
        user_id,
        "payment.recorded",
        "payment",
        resp.id,
        audit_snapshot,
        serde_json::json!({ "invoice_id": invoice_id }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// List payments
// ══════════════════════════════════════════════════════════════════════════════

pub async fn list_payments(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    invoice_id: i64,
) -> AppResult<Vec<PaymentResponse>> {
    invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    let payments = payment_entity::Entity::find()
        .filter(payment_entity::Column::InvoiceId.eq(invoice_id))
        .order_by_asc(payment_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut responses = Vec::with_capacity(payments.len());
    for p in payments {
        responses.push(load_payment_details(conn, cipher, p).await?);
    }
    Ok(responses)
}

// ══════════════════════════════════════════════════════════════════════════════
// Handle exception
// ══════════════════════════════════════════════════════════════════════════════

pub async fn handle_exception(
    conn: &DatabaseConnection,
    cipher: &Cipher,
    invoice_id: i64,
    payment_id: i64,
    user_id: i64,
    req: HandleExceptionRequest,
) -> AppResult<PaymentResponse> {
    if req.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".into()));
    }

    let exc_type = parse_exception_type(&req.exception_type)?;

    let new_status = match exc_type {
        ExceptionType::Void => PaymentEntryStatus::Voided,
        ExceptionType::Reversal => PaymentEntryStatus::Reversed,
        ExceptionType::Dispute => PaymentEntryStatus::Disputed,
    };

    let payment = payment_entity::Entity::find_by_id(payment_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Payment {payment_id} not found")))?;

    if payment.invoice_id != invoice_id {
        return Err(AppError::NotFound(format!(
            "Payment {payment_id} does not belong to invoice {invoice_id}"
        )));
    }

    if payment.status != PaymentEntryStatus::Active {
        return Err(AppError::Conflict(format!(
            "Payment {payment_id} is already '{}'; exceptions can only be raised on active payments",
            status_str(&payment.status)
        )));
    }

    let now = Utc::now();

    let updated_payment = conn
        .transaction::<_, payment_entity::Model, AppError>(|txn| {
            let payment = payment.clone();
            let exc_type = exc_type.clone();
            let new_status = new_status.clone();
            let reason = req.reason.trim().to_owned();
            Box::pin(async move {
                ExceptionActiveModel {
                    payment_id: Set(payment_id),
                    exception_type: Set(exc_type),
                    reason: Set(reason),
                    raised_by: Set(user_id),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

                let mut active: PaymentActiveModel = payment.into();
                active.status = Set(new_status);
                let updated = active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?;

                let inv = invoice_entity::Entity::find_by_id(invoice_id)
                    .one(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))?
                    .ok_or_else(|| AppError::Internal("Invoice vanished mid-transaction".into()))?;

                if inv.status == InvoiceStatus::Paid {
                    let paid_total = effective_paid_total(txn, invoice_id)
                        .await
                        .map_err(|e| AppError::Internal(e.to_string()))?;

                    if paid_total < inv.total {
                        let mut inv_active: InvoiceActiveModel = inv.into();
                        inv_active.status = Set(InvoiceStatus::Issued);
                        inv_active.updated_at = Set(now);
                        inv_active
                            .update(txn)
                            .await
                            .map_err(|e| AppError::Internal(e.to_string()))?;
                    }
                }

                Ok(updated)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        payment_id,
        invoice_id,
        user_id,
        exception_type = %req.exception_type,
        "billing.payment_exception_raised"
    );

    let resp = load_payment_details(conn, cipher, updated_payment).await?;
    let audit_snapshot = {
        let mut s = serde_json::to_value(&resp).unwrap_or_default();
        if let Some(obj) = s.as_object_mut() {
            obj.insert(
                "external_reference".into(),
                serde_json::Value::String(Cipher::mask().to_owned()),
            );
        }
        s
    };
    audit::service::append(
        conn,
        user_id,
        "payment.exception_raised",
        "payment",
        resp.id,
        audit_snapshot,
        serde_json::json!({
            "invoice_id":     invoice_id,
            "exception_type": req.exception_type,
            "reason":         req.reason,
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Request refund
// ══════════════════════════════════════════════════════════════════════════════

pub async fn request_refund(
    conn: &DatabaseConnection,
    invoice_id: i64,
    payment_id: i64,
    user_id: i64,
    req: RequestRefundRequest,
) -> AppResult<RefundResponse> {
    if req.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".into()));
    }
    if req.amount <= 0.0 || !req.amount.is_finite() {
        return Err(AppError::BadRequest(
            "amount must be a positive finite number".into(),
        ));
    }

    let refund_amount = dec(req.amount)?;

    let payment = payment_entity::Entity::find_by_id(payment_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Payment {payment_id} not found")))?;

    if payment.invoice_id != invoice_id {
        return Err(AppError::NotFound(format!(
            "Payment {payment_id} does not belong to invoice {invoice_id}"
        )));
    }

    if payment.status != PaymentEntryStatus::Active {
        return Err(AppError::Conflict(format!(
            "Cannot refund payment {payment_id}: status is '{}'",
            status_str(&payment.status)
        )));
    }

    if refund_amount > payment.amount {
        return Err(AppError::UnprocessableEntity(format!(
            "Refund amount {} exceeds payment amount {}",
            refund_amount, payment.amount
        )));
    }

    let now = Utc::now();

    let refund = RefundActiveModel {
        payment_id: Set(payment_id),
        invoice_line_id: Set(None),
        amount: Set(refund_amount),
        reason: Set(req.reason.trim().to_owned()),
        status: Set(RefundStatus::PendingFinance),
        requested_by: Set(user_id),
        finance_approved_by: Set(None),
        auditor_approved_by: Set(None),
        rejected_by: Set(None),
        rejection_reason: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        refund_id = refund.id,
        payment_id,
        invoice_id,
        user_id,
        amount = %refund_amount,
        "billing.refund_requested"
    );

    let resp = refund_to_response(&refund);
    audit::service::append(
        conn,
        user_id,
        "payment.refund_requested",
        "payment_refund",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "invoice_id": invoice_id,
            "payment_id": payment_id,
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Approve refund
// ══════════════════════════════════════════════════════════════════════════════

pub async fn approve_refund(
    conn: &DatabaseConnection,
    invoice_id: i64,
    payment_id: i64,
    refund_id: i64,
    user: &AuthenticatedUser,
    _req: ApproveRefundRequest,
) -> AppResult<RefundResponse> {
    let refund = load_refund(conn, payment_id, refund_id, invoice_id).await?;

    let now = Utc::now();

    // Extract permission checks and user_id before the async closure so that
    // the closure captures only 'static / Copy values (no &AuthenticatedUser).
    let has_financials_write = user.has_permission(Permission::FinancialsWrite);
    let has_audit_read = user.has_permission(Permission::AuditRead);
    let user_id = user.user_id;

    let updated = conn
        .transaction::<_, refund_entity::Model, AppError>(|txn| {
            let refund = refund.clone();
            Box::pin(async move {
                let new_refund = match refund.status {
                    RefundStatus::PendingFinance => {
                        if !has_financials_write {
                            return Err(AppError::Forbidden(
                                "Finance Clerk permission (financials:write) required \
                                 to approve at this stage"
                                    .into(),
                            ));
                        }

                        if refund.amount > AUDITOR_THRESHOLD {
                            let mut active: RefundActiveModel = refund.into();
                            active.status = Set(RefundStatus::PendingAuditor);
                            active.finance_approved_by = Set(Some(user_id));
                            active.updated_at = Set(now);
                            active
                                .update(txn)
                                .await
                                .map_err(|e| AppError::Internal(e.to_string()))?
                        } else {
                            let mut active: RefundActiveModel = refund.clone().into();
                            active.status = Set(RefundStatus::Approved);
                            active.finance_approved_by = Set(Some(user_id));
                            active.updated_at = Set(now);
                            let updated = active
                                .update(txn)
                                .await
                                .map_err(|e| AppError::Internal(e.to_string()))?;

                            insert_refund_line(txn, invoice_id, &updated, now).await?;

                            refund_entity::Entity::find_by_id(updated.id)
                                .one(txn)
                                .await
                                .map_err(|e| AppError::Internal(e.to_string()))?
                                .ok_or_else(|| AppError::Internal("Refund vanished".into()))?
                        }
                    }
                    RefundStatus::PendingAuditor => {
                        if !has_audit_read {
                            return Err(AppError::Forbidden(
                                "Auditor permission (audit:read) required to approve at this stage"
                                    .into(),
                            ));
                        }

                        let mut active: RefundActiveModel = refund.clone().into();
                        active.status = Set(RefundStatus::Approved);
                        active.auditor_approved_by = Set(Some(user_id));
                        active.updated_at = Set(now);
                        let updated = active
                            .update(txn)
                            .await
                            .map_err(|e| AppError::Internal(e.to_string()))?;

                        insert_refund_line(txn, invoice_id, &updated, now).await?;

                        refund_entity::Entity::find_by_id(updated.id)
                            .one(txn)
                            .await
                            .map_err(|e| AppError::Internal(e.to_string()))?
                            .ok_or_else(|| AppError::Internal("Refund vanished".into()))?
                    }
                    RefundStatus::Approved => {
                        return Err(AppError::Conflict(format!(
                            "Refund {refund_id} is already approved"
                        )));
                    }
                    RefundStatus::Rejected => {
                        return Err(AppError::Conflict(format!(
                            "Refund {refund_id} has been rejected and cannot be approved"
                        )));
                    }
                };

                Ok(new_refund)
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        refund_id,
        payment_id,
        invoice_id,
        user_id = user.user_id,
        new_status = %refund_status_str(&updated.status),
        "billing.refund_approved"
    );

    let resp = refund_to_response(&updated);
    audit::service::append(
        conn,
        user.user_id,
        "payment.refund_approved",
        "payment_refund",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "invoice_id": invoice_id,
            "payment_id": payment_id,
        }),
    )
    .await?;
    Ok(resp)
}

async fn insert_refund_line(
    txn: &impl sea_orm::ConnectionTrait,
    invoice_id: i64,
    refund: &refund_entity::Model,
    now: DateTime<Utc>,
) -> AppResult<()> {
    use crate::entity::enums::PricingModel;

    let line = LineActiveModel {
        invoice_id: Set(invoice_id),
        description: Set(format!("Refund: {}", refund.reason)),
        pricing_model: Set(PricingModel::Fixed),
        quantity: Set(1.0),
        unit_price: Set(Decimal::ZERO),
        adjustment_type: Set(None),
        adjustment_is_percentage: Set(false),
        adjustment_value: Set(None),
        line_total: Set(-refund.amount),
        is_refund: Set(true),
        created_at: Set(now),
        ..Default::default()
    }
    .insert(txn)
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?;

    let mut r_active: RefundActiveModel = refund.clone().into();
    r_active.invoice_line_id = Set(Some(line.id));
    r_active
        .update(txn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    recompute_invoice_totals(txn, invoice_id, now).await
}

async fn recompute_invoice_totals(
    txn: &impl sea_orm::ConnectionTrait,
    invoice_id: i64,
    now: DateTime<Utc>,
) -> AppResult<()> {
    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(txn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::Internal("Invoice vanished mid-transaction".into()))?;

    let lines = line_entity::Entity::find()
        .filter(line_entity::Column::InvoiceId.eq(invoice_id))
        .all(txn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    let subtotal = lines
        .iter()
        .fold(Decimal::ZERO, |acc, l| acc + l.line_total)
        .round_dp(MONEY_DP);

    let tax = (subtotal * inv.tax_rate).round_dp(MONEY_DP);

    let discount_amount = match (inv.discount_type.as_deref(), inv.discount_value) {
        (Some("percentage"), Some(pct)) => {
            let raw = (subtotal * (pct / Decimal::ONE_HUNDRED)).round_dp(MONEY_DP);
            raw.min(Decimal::from_parts(500, 0, 0, false, 0))
        }
        (Some("fixed_amount"), Some(amt)) => amt
            .round_dp(MONEY_DP)
            .min(Decimal::from_parts(500, 0, 0, false, 0)),
        _ => inv.discount_amount,
    };

    let total = (subtotal + tax - discount_amount).max(Decimal::ZERO);

    let mut active: InvoiceActiveModel = inv.into();
    active.subtotal = Set(subtotal);
    active.tax = Set(tax);
    active.discount_amount = Set(discount_amount);
    active.total = Set(total);
    active.updated_at = Set(now);
    active
        .update(txn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    Ok(())
}

// ══════════════════════════════════════════════════════════════════════════════
// Reject refund
// ══════════════════════════════════════════════════════════════════════════════

pub async fn reject_refund(
    conn: &DatabaseConnection,
    invoice_id: i64,
    payment_id: i64,
    refund_id: i64,
    user: &AuthenticatedUser,
    req: RejectRefundRequest,
) -> AppResult<RefundResponse> {
    if req.reason.trim().is_empty() {
        return Err(AppError::BadRequest("reason is required".into()));
    }

    let refund = load_refund(conn, payment_id, refund_id, invoice_id).await?;

    match refund.status {
        RefundStatus::PendingFinance => {
            if !user.has_permission(Permission::FinancialsWrite) {
                return Err(AppError::Forbidden(
                    "Finance Clerk permission required to reject at this stage".into(),
                ));
            }
        }
        RefundStatus::PendingAuditor => {
            if !user.has_permission(Permission::FinancialsWrite)
                && !user.has_permission(Permission::AuditRead)
            {
                return Err(AppError::Forbidden(
                    "Finance Clerk or Auditor permission required to reject at this stage".into(),
                ));
            }
        }
        RefundStatus::Approved => {
            return Err(AppError::Conflict(format!(
                "Refund {refund_id} is already approved and cannot be rejected"
            )));
        }
        RefundStatus::Rejected => {
            return Err(AppError::Conflict(format!(
                "Refund {refund_id} is already rejected"
            )));
        }
    }

    let now = Utc::now();
    let mut active: RefundActiveModel = refund.into();
    active.status = Set(RefundStatus::Rejected);
    active.rejected_by = Set(Some(user.user_id));
    active.rejection_reason = Set(Some(req.reason.trim().to_owned()));
    active.updated_at = Set(now);

    let updated = active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        refund_id,
        payment_id,
        invoice_id,
        user_id = user.user_id,
        "billing.refund_rejected"
    );

    let resp = refund_to_response(&updated);
    audit::service::append(
        conn,
        user.user_id,
        "payment.refund_rejected",
        "payment_refund",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "invoice_id": invoice_id,
            "payment_id": payment_id,
            "reason":     req.reason,
        }),
    )
    .await?;
    Ok(resp)
}

// ── Shared loader ─────────────────────────────────────────────────────────────

async fn load_refund(
    conn: &DatabaseConnection,
    payment_id: i64,
    refund_id: i64,
    invoice_id: i64,
) -> AppResult<refund_entity::Model> {
    let payment = payment_entity::Entity::find_by_id(payment_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Payment {payment_id} not found")))?;

    if payment.invoice_id != invoice_id {
        return Err(AppError::NotFound(format!(
            "Payment {payment_id} does not belong to invoice {invoice_id}"
        )));
    }

    refund_entity::Entity::find_by_id(refund_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Refund {refund_id} not found")))
        .and_then(|r| {
            if r.payment_id != payment_id {
                Err(AppError::NotFound(format!(
                    "Refund {refund_id} does not belong to payment {payment_id}"
                )))
            } else {
                Ok(r)
            }
        })
}

// ══════════════════════════════════════════════════════════════════════════════
// Native Rust unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;
    use crate::entity::enums::{
        ExceptionType, PaymentEntryStatus, PaymentMethod, RefundStatus,
    };

    // ── parse_method ────────────────────────────────────────────────────────

    #[test]
    fn parse_method_all_valid() {
        assert_eq!(parse_method("cash").unwrap(),          PaymentMethod::Cash);
        assert_eq!(parse_method("cheque").unwrap(),        PaymentMethod::Cheque);
        assert_eq!(parse_method("check").unwrap(),         PaymentMethod::Cheque);
        assert_eq!(parse_method("ach").unwrap(),           PaymentMethod::Ach);
        assert_eq!(parse_method("bank_transfer").unwrap(), PaymentMethod::BankTransfer);
        assert_eq!(parse_method("card").unwrap(),          PaymentMethod::Card);
    }

    #[test]
    fn parse_method_rejects_unknown() {
        assert!(parse_method("crypto").is_err());
        assert!(parse_method("").is_err());
    }

    #[test]
    fn method_str_roundtrips() {
        let variants = [
            PaymentMethod::Cash, PaymentMethod::Cheque, PaymentMethod::Ach,
            PaymentMethod::BankTransfer, PaymentMethod::Card,
        ];
        for v in variants {
            let s = method_str(&v);
            assert_eq!(parse_method(s).unwrap(), v, "roundtrip failed for {s}");
        }
    }

    // ── parse_exception_type ────────────────────────────────────────────────

    #[test]
    fn parse_exception_type_all_valid() {
        assert_eq!(parse_exception_type("void").unwrap(),     ExceptionType::Void);
        assert_eq!(parse_exception_type("reversal").unwrap(), ExceptionType::Reversal);
        assert_eq!(parse_exception_type("dispute").unwrap(),  ExceptionType::Dispute);
    }

    #[test]
    fn parse_exception_type_rejects_unknown() {
        assert!(parse_exception_type("write_off").is_err());
    }

    #[test]
    fn exception_type_str_roundtrips() {
        let variants = [ExceptionType::Void, ExceptionType::Reversal, ExceptionType::Dispute];
        for v in variants {
            let s = exception_type_str(&v);
            assert_eq!(parse_exception_type(s).unwrap(), v);
        }
    }

    // ── status / refund status string coverage ──────────────────────────────

    #[test]
    fn status_str_covers_all_variants() {
        assert_eq!(status_str(&PaymentEntryStatus::Active),   "active");
        assert_eq!(status_str(&PaymentEntryStatus::Voided),   "voided");
        assert_eq!(status_str(&PaymentEntryStatus::Reversed), "reversed");
        assert_eq!(status_str(&PaymentEntryStatus::Disputed), "disputed");
    }

    #[test]
    fn refund_status_str_covers_all_variants() {
        assert_eq!(refund_status_str(&RefundStatus::PendingFinance), "pending_finance");
        assert_eq!(refund_status_str(&RefundStatus::PendingAuditor), "pending_auditor");
        assert_eq!(refund_status_str(&RefundStatus::Approved),       "approved");
        assert_eq!(refund_status_str(&RefundStatus::Rejected),       "rejected");
    }

    // ── parse_datetime ──────────────────────────────────────────────────────

    #[test]
    fn parse_datetime_valid_rfc3339() {
        let dt = parse_datetime("2026-01-15T12:00:00Z").unwrap();
        assert_eq!(dt.year(), 2026);
        assert_eq!(dt.month(), 1);
        assert_eq!(dt.day(), 15);
    }

    #[test]
    fn parse_datetime_rejects_garbage() {
        assert!(parse_datetime("not-a-date").is_err());
    }

    // ── dec helper ──────────────────────────────────────────────────────────

    #[test]
    fn dec_positive() {
        let d = dec(123.45).unwrap();
        assert_eq!(d, Decimal::from_str_exact("123.45").unwrap());
    }

    #[test]
    fn dec_zero() {
        let d = dec(0.0).unwrap();
        assert_eq!(d, Decimal::ZERO);
    }
}
