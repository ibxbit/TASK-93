pub mod handlers;
pub mod service;

use serde::{Deserialize, Serialize};

// ── Create invoice ────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct CreateInvoiceRequest {
    /// Human-readable reference, e.g. "INV-2024-0042". Must be globally unique.
    pub invoice_no: String,
    pub counterparty: String,
    /// ISO 8601 date string "YYYY-MM-DD".
    pub issue_date: String,
    /// Tax rate as a fraction: 0.0 = no tax, 0.10 = 10 %, 0.20 = 20 %.
    /// Must be in [0, 1].
    pub tax_rate: f64,
}

// ── Add line item ─────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct AddLineRequest {
    pub description: String,
    /// "per_unit" | "per_duration" | "package" | "fixed" | "percentage"
    pub pricing_model: String,
    /// Number of units, hours, packages, etc.  Must be > 0.
    pub quantity: f64,
    /// Price per unit / duration / package in dollars.
    pub unit_price: f64,
    /// Optional line-level adjustment: "discount" | "surcharge".
    pub adjustment_type: Option<String>,
    /// Percentage value (0–30 for discounts, ≥ 0 for surcharges) if
    /// `adjustment_is_percentage` is true; otherwise a fixed dollar amount.
    pub adjustment_value: Option<f64>,
    /// When `true`, `adjustment_value` is treated as a percentage of the base
    /// line amount.  When `false` (default), it is a fixed dollar amount.
    #[serde(default)]
    pub adjustment_is_percentage: bool,
}

// ── Apply discount ────────────────────────────────────────────────────────────

#[derive(Deserialize, Clone)]
pub struct ApplyDiscountRequest {
    /// "percentage" — `value` is a percentage of the subtotal (0–30 %).
    /// "fixed_amount" — `value` is a dollar amount (≥ 0).
    pub discount_type: String,
    /// For "percentage": a number in [0, 30].
    /// For "fixed_amount": a dollar amount ≥ 0.
    /// The computed discount is capped at $500 per invoice.
    pub discount_value: f64,
}

// ── Issue invoice ─────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct IssueInvoiceRequest {
    // No fields required — transition from draft to issued.
}

// ── Response types ────────────────────────────────────────────────────────────

/// All monetary values are returned as decimal strings to preserve precision.
#[derive(Serialize, Clone)]
pub struct LineResponse {
    pub id: i64,
    pub invoice_id: i64,
    pub description: String,
    pub pricing_model: String,
    pub quantity: f64,
    pub unit_price: String,
    pub adjustment_type: Option<String>,
    pub adjustment_is_percentage: bool,
    pub adjustment_value: Option<String>,
    pub line_total: String,
    pub is_refund: bool,
    pub created_at: String,
}

#[derive(Serialize)]
pub struct InvoiceResponse {
    pub id: i64,
    pub invoice_no: String,
    pub counterparty: String,
    pub issue_date: String,
    pub tax_rate: String,
    pub subtotal: String,
    pub tax: String,
    pub discount_type: Option<String>,
    pub discount_value: Option<String>,
    pub discount_amount: String,
    pub total: String,
    pub status: String,
    pub created_by: i64,
    pub created_at: String,
    pub updated_at: String,
    pub lines: Vec<LineResponse>,
}

// ── Query filter ──────────────────────────────────────────────────────────────

#[derive(rocket::FromForm)]
pub struct InvoiceFilterQuery {
    /// "draft" | "issued" | "paid" | "cancelled" | "overdue"
    pub status: Option<String>,
    pub counterparty: Option<String>,
}
