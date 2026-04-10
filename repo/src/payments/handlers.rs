use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::crypto::Cipher;
use crate::errors::AppResult;
use crate::rbac::guards::RequireFinancialsRead;
use crate::rbac::guards::RequireFinancialsWrite;

use super::{
    service, ApproveRefundRequest, ExceptionResponse, HandleExceptionRequest, PaymentResponse,
    RecordPaymentRequest, RefundResponse, RejectRefundRequest, RequestRefundRequest,
};

// ── Record / list payments ────────────────────────────────────────────────────

/// Record a manual payment against an issued invoice.
///
/// `external_reference` is AES-256-GCM encrypted at rest; the plaintext value
/// is returned in the response but never stored in cleartext.
///
/// Idempotency: the same `external_reference` submitted for the same invoice a
/// second time returns the existing payment unchanged (200).  Submitting the
/// same reference for a different invoice returns 409.
///
/// **Required permission:** `financials:write`
#[post("/invoices/<invoice_id>/payments", data = "<body>")]
pub async fn record_payment(
    guard: RequireFinancialsWrite,
    invoice_id: i64,
    body: Json<RecordPaymentRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<PaymentResponse>> {
    let resp = service::record_payment(
        conn.inner(),
        cipher.inner(),
        invoice_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// List all payments for an invoice, oldest first.
///
/// `external_reference` values are decrypted before being returned.
///
/// **Required permission:** `financials:read`
#[get("/invoices/<invoice_id>/payments")]
pub async fn list_payments(
    _guard: RequireFinancialsRead,
    invoice_id: i64,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<Vec<PaymentResponse>>> {
    Ok(Json(
        service::list_payments(conn.inner(), cipher.inner(), invoice_id).await?,
    ))
}

// ── Exception handling ────────────────────────────────────────────────────────

/// Raise an exception (void / reversal / dispute) against an active payment.
///
/// **Required permission:** `financials:write`
#[post(
    "/invoices/<invoice_id>/payments/<payment_id>/exceptions",
    data = "<body>"
)]
pub async fn handle_exception(
    guard: RequireFinancialsWrite,
    invoice_id: i64,
    payment_id: i64,
    body: Json<HandleExceptionRequest>,
    conn: &State<DatabaseConnection>,
    cipher: &State<Cipher>,
) -> AppResult<Json<PaymentResponse>> {
    let resp = service::handle_exception(
        conn.inner(),
        cipher.inner(),
        invoice_id,
        payment_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

// ── Refund workflow ───────────────────────────────────────────────────────────

/// Request a refund against an active payment.
///
/// **Required permission:** `financials:write`
#[post(
    "/invoices/<invoice_id>/payments/<payment_id>/refunds",
    data = "<body>"
)]
pub async fn request_refund(
    guard: RequireFinancialsWrite,
    invoice_id: i64,
    payment_id: i64,
    body: Json<RequestRefundRequest>,
    conn: &State<DatabaseConnection>,
    _cipher: &State<Cipher>,
) -> AppResult<Json<RefundResponse>> {
    let resp = service::request_refund(
        conn.inner(),
        invoice_id,
        payment_id,
        guard.0.user_id,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Approve a pending refund.
///
/// **Required permission:** `financials:read` (minimum; role-specific checks inside)
#[post(
    "/invoices/<invoice_id>/payments/<payment_id>/refunds/<refund_id>/approve",
    data = "<body>"
)]
pub async fn approve_refund(
    guard: RequireFinancialsRead,
    invoice_id: i64,
    payment_id: i64,
    refund_id: i64,
    body: Json<ApproveRefundRequest>,
    conn: &State<DatabaseConnection>,
    _cipher: &State<Cipher>,
) -> AppResult<Json<RefundResponse>> {
    let resp = service::approve_refund(
        conn.inner(),
        invoice_id,
        payment_id,
        refund_id,
        &guard.0,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// Reject a pending refund.
///
/// **Required permission:** `financials:read` (minimum; role-specific checks inside)
#[post(
    "/invoices/<invoice_id>/payments/<payment_id>/refunds/<refund_id>/reject",
    data = "<body>"
)]
pub async fn reject_refund(
    guard: RequireFinancialsRead,
    invoice_id: i64,
    payment_id: i64,
    refund_id: i64,
    body: Json<RejectRefundRequest>,
    conn: &State<DatabaseConnection>,
    _cipher: &State<Cipher>,
) -> AppResult<Json<RefundResponse>> {
    let resp = service::reject_refund(
        conn.inner(),
        invoice_id,
        payment_id,
        refund_id,
        &guard.0,
        body.into_inner(),
    )
    .await?;
    Ok(Json(resp))
}

/// List all exception records for a payment.
///
/// **Required permission:** `financials:read`
#[get("/invoices/<invoice_id>/payments/<payment_id>/exceptions")]
pub async fn list_exceptions(
    _guard: RequireFinancialsRead,
    invoice_id: i64,
    payment_id: i64,
    conn: &State<DatabaseConnection>,
    _cipher: &State<Cipher>,
) -> AppResult<Json<Vec<ExceptionResponse>>> {
    use crate::entity::payment_exception as exception_entity;
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

    use crate::entity::payment_entry as payment_entity;
    let payment = payment_entity::Entity::find_by_id(payment_id)
        .one(conn.inner())
        .await
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?
        .ok_or_else(|| {
            crate::errors::AppError::NotFound(format!("Payment {payment_id} not found"))
        })?;

    if payment.invoice_id != invoice_id {
        return Err(crate::errors::AppError::NotFound(format!(
            "Payment {payment_id} does not belong to invoice {invoice_id}"
        )));
    }

    let exceptions = exception_entity::Entity::find()
        .filter(exception_entity::Column::PaymentId.eq(payment_id))
        .order_by_asc(exception_entity::Column::CreatedAt)
        .all(conn.inner())
        .await
        .map_err(|e| crate::errors::AppError::Internal(e.to_string()))?;

    Ok(Json(
        exceptions
            .iter()
            .map(|e| ExceptionResponse {
                id: e.id,
                payment_id: e.payment_id,
                exception_type: match &e.exception_type {
                    crate::entity::enums::ExceptionType::Void => "void".into(),
                    crate::entity::enums::ExceptionType::Reversal => "reversal".into(),
                    crate::entity::enums::ExceptionType::Dispute => "dispute".into(),
                },
                reason: e.reason.clone(),
                raised_by: e.raised_by,
                created_at: e.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}
