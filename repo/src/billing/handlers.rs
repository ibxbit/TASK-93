use rocket::{serde::json::Json, State};
use sea_orm::DatabaseConnection;

use crate::errors::AppResult;
use crate::rbac::guards::{RequireFinancialsRead, RequireFinancialsWrite};

use super::{
    service, AddLineRequest, ApplyDiscountRequest, CreateInvoiceRequest, InvoiceFilterQuery,
    InvoiceResponse, IssueInvoiceRequest,
};

// ── Create ────────────────────────────────────────────────────────────────────

/// Create a new draft invoice.
///
/// `invoice_no` must be globally unique.  `tax_rate` is a fraction in [0, 1]
/// (e.g. `0.10` for 10 %).  The invoice starts with zero line items; add lines
/// before issuing.
///
/// **Required permission:** `financials:write`
#[post("/invoices", data = "<body>")]
pub async fn create_invoice(
    guard: RequireFinancialsWrite,
    body: Json<CreateInvoiceRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<InvoiceResponse>> {
    let resp = service::create_invoice(conn.inner(), guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

// ── Read ──────────────────────────────────────────────────────────────────────

/// Get a single invoice by ID, including all line items and computed totals.
///
/// **Required permission:** `financials:read`
#[get("/invoices/<id>")]
pub async fn get_invoice(
    _guard: RequireFinancialsRead,
    id: i64,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<InvoiceResponse>> {
    Ok(Json(service::get_invoice(conn.inner(), id).await?))
}

/// List invoices with optional `status` and/or `counterparty` query filters.
///
/// Results are ordered newest-first.
///
/// **Required permission:** `financials:read`
#[get("/invoices?<filter..>")]
pub async fn list_invoices(
    _guard: RequireFinancialsRead,
    filter: InvoiceFilterQuery,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<Vec<InvoiceResponse>>> {
    Ok(Json(service::list_invoices(conn.inner(), filter).await?))
}

// ── Line items ────────────────────────────────────────────────────────────────

/// Add a line item to a draft invoice.
///
/// Supported pricing models: `per_unit`, `per_duration`, `package`, `fixed`,
/// `percentage`.  An optional line-level `adjustment_type` (`discount` or
/// `surcharge`) can be supplied with an `adjustment_value`.  Set
/// `adjustment_is_percentage: true` to treat the value as a percentage of the
/// base line amount; otherwise it is a fixed dollar amount.
///
/// Invoice subtotal, tax, discount, and total are recomputed atomically after
/// the line is inserted.  Only draft invoices accept new lines.
///
/// **Required permission:** `financials:write`
#[post("/invoices/<id>/lines", data = "<body>")]
pub async fn add_line(
    guard: RequireFinancialsWrite,
    id: i64,
    body: Json<AddLineRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<InvoiceResponse>> {
    let resp = service::add_line(conn.inner(), id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

// ── Discount ──────────────────────────────────────────────────────────────────

/// Apply an invoice-level discount to a draft invoice.
///
/// `discount_type` must be `"percentage"` (0–30 %) or `"fixed_amount"` (≥ 0).
/// The computed discount is capped at $500 per invoice regardless of type.
/// Calling this endpoint again replaces the previously stored discount.
///
/// **Required permission:** `financials:write`
#[post("/invoices/<id>/discount", data = "<body>")]
pub async fn apply_discount(
    guard: RequireFinancialsWrite,
    id: i64,
    body: Json<ApplyDiscountRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<InvoiceResponse>> {
    let resp =
        service::apply_discount(conn.inner(), id, guard.0.user_id, body.into_inner()).await?;
    Ok(Json(resp))
}

// ── Issue ─────────────────────────────────────────────────────────────────────

/// Transition a draft invoice to `issued`.
///
/// The invoice must have at least one line item (non-zero subtotal).  Once
/// issued, line items and discounts can no longer be modified.
///
/// **Required permission:** `financials:write`
#[post("/invoices/<id>/issue", data = "<body>")]
pub async fn issue_invoice(
    guard: RequireFinancialsWrite,
    id: i64,
    _body: Json<IssueInvoiceRequest>,
    conn: &State<DatabaseConnection>,
) -> AppResult<Json<InvoiceResponse>> {
    let resp = service::issue_invoice(conn.inner(), id, guard.0.user_id).await?;
    Ok(Json(resp))
}
