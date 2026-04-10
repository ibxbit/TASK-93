pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Record payment ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct RecordPaymentRequest {
    /// "cash" | "cheque" | "ach"
    pub method: String,
    /// Payment amount in dollars. Must be > 0.
    pub amount: f64,
    /// Unique idempotency key (e.g. ACH trace number, cheque number, cash receipt
    /// ID). A second submission with the same key for the same invoice is silently
    /// de-duplicated; the same key for a different invoice returns 409.
    pub external_reference: String,
    /// ISO 8601 date-time the payment was physically received, e.g.
    /// "2024-07-01T14:30:00Z".
    pub received_at: String,
    pub notes: Option<String>,
}

// ── Handle exception ───────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct HandleExceptionRequest {
    /// "void" | "reversal" | "dispute"
    pub exception_type: String,
    /// Mandatory explanation (e.g. bank memo, dispute case number).
    pub reason: String,
}

// ── Refund ─────────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct RequestRefundRequest {
    /// Dollar amount to refund. Must be > 0 and ≤ payment amount.
    pub amount: f64,
    pub reason: String,
}

#[derive(Deserialize)]
pub struct ApproveRefundRequest {
    // No required fields — the caller's identity and role determine approval authority.
}

#[derive(Deserialize)]
pub struct RejectRefundRequest {
    pub reason: String,
}

// ── Responses ──────────────────────────────────────────────────────────────────

/// All monetary amounts are returned as decimal strings.
#[derive(Serialize)]
pub struct PaymentResponse {
    pub id: i64,
    pub invoice_id: i64,
    pub method: String,
    pub amount: String,
    pub external_reference: String,
    pub received_at: String,
    pub notes: Option<String>,
    pub status: String,
    pub recorded_by: i64,
    pub created_at: String,
    pub exceptions: Vec<ExceptionResponse>,
    pub refunds: Vec<RefundResponse>,
}

#[derive(Serialize, Clone)]
pub struct ExceptionResponse {
    pub id: i64,
    pub payment_id: i64,
    pub exception_type: String,
    pub reason: String,
    pub raised_by: i64,
    pub created_at: String,
}

#[derive(Serialize, Clone)]
pub struct RefundResponse {
    pub id: i64,
    pub payment_id: i64,
    pub invoice_line_id: Option<i64>,
    pub amount: String,
    pub reason: String,
    pub status: String,
    pub requested_by: i64,
    pub finance_approved_by: Option<i64>,
    pub auditor_approved_by: Option<i64>,
    pub rejected_by: Option<i64>,
    pub rejection_reason: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}
