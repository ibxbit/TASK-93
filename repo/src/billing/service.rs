use chrono::{NaiveDate, Utc};
use rust_decimal::prelude::*;
use rust_decimal::Decimal;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionError, TransactionTrait,
};

use crate::audit;
use crate::entity::enums::{AdjustmentType, InvoiceStatus, PricingModel};
use crate::entity::{
    invoice::{self as invoice_entity, ActiveModel as InvoiceActiveModel},
    invoice_line::{self as line_entity, ActiveModel as LineActiveModel},
};
use crate::errors::{AppError, AppResult};

use super::{
    AddLineRequest, ApplyDiscountRequest, CreateInvoiceRequest, InvoiceFilterQuery,
    InvoiceResponse, LineResponse,
};

// ── Constants ─────────────────────────────────────────────────────────────────

/// Maximum percentage discount allowed per invoice (30 %).
const MAX_DISCOUNT_PCT: Decimal = Decimal::from_parts(30, 0, 0, false, 0);

/// Maximum computed discount dollar amount per invoice ($500.00).
const MAX_DISCOUNT_AMOUNT: Decimal = Decimal::from_parts(500, 0, 0, false, 0);

/// Decimal scale used for all monetary rounding (4 decimal places).
const MONEY_DP: u32 = 4;

// ── Parsing helpers ───────────────────────────────────────────────────────────

fn parse_pricing_model(s: &str) -> AppResult<PricingModel> {
    match s {
        "fixed" => Ok(PricingModel::Fixed),
        "per_unit" => Ok(PricingModel::PerUnit),
        "percentage" => Ok(PricingModel::Percentage),
        "per_duration" => Ok(PricingModel::PerDuration),
        "package" => Ok(PricingModel::Package),
        _ => Err(AppError::BadRequest(format!(
            "Unknown pricing_model '{s}'. Valid: per_unit, per_duration, package, fixed, percentage"
        ))),
    }
}

fn pricing_model_str(m: &PricingModel) -> &'static str {
    match m {
        PricingModel::Fixed => "fixed",
        PricingModel::PerUnit => "per_unit",
        PricingModel::Percentage => "percentage",
        PricingModel::PerDuration => "per_duration",
        PricingModel::Package => "package",
    }
}

fn parse_adjustment_type(s: &str) -> AppResult<AdjustmentType> {
    match s {
        "discount" => Ok(AdjustmentType::Discount),
        "surcharge" => Ok(AdjustmentType::Surcharge),
        _ => Err(AppError::BadRequest(format!(
            "Unknown adjustment_type '{s}'. Valid: discount, surcharge"
        ))),
    }
}

fn adjustment_type_str(a: &AdjustmentType) -> &'static str {
    match a {
        AdjustmentType::Discount => "discount",
        AdjustmentType::Surcharge => "surcharge",
    }
}

fn status_str(s: &InvoiceStatus) -> &'static str {
    match s {
        InvoiceStatus::Draft => "draft",
        InvoiceStatus::Issued => "issued",
        InvoiceStatus::Paid => "paid",
        InvoiceStatus::Cancelled => "cancelled",
        InvoiceStatus::Overdue => "overdue",
    }
}

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest(format!("Invalid issue_date '{s}'; expected YYYY-MM-DD")))
}

/// Convert `f64` to `Decimal`, returning a `BadRequest` error on NaN / infinity.
fn dec(v: f64) -> AppResult<Decimal> {
    Decimal::from_f64(v).ok_or_else(|| AppError::BadRequest(format!("Invalid numeric value: {v}")))
}

fn tx_err(e: TransactionError<AppError>) -> AppError {
    match e {
        TransactionError::Transaction(e) => e,
        TransactionError::Connection(e) => AppError::Internal(e.to_string()),
    }
}

// ── Line → response ───────────────────────────────────────────────────────────

fn line_to_response(m: &line_entity::Model) -> LineResponse {
    LineResponse {
        id: m.id,
        invoice_id: m.invoice_id,
        description: m.description.clone(),
        pricing_model: pricing_model_str(&m.pricing_model).to_owned(),
        quantity: m.quantity,
        unit_price: m.unit_price.to_string(),
        adjustment_type: m
            .adjustment_type
            .as_ref()
            .map(adjustment_type_str)
            .map(str::to_owned),
        adjustment_is_percentage: m.adjustment_is_percentage,
        adjustment_value: m.adjustment_value.map(|v| v.to_string()),
        line_total: m.line_total.to_string(),
        is_refund: m.is_refund,
        created_at: m.created_at.to_rfc3339(),
    }
}

fn invoice_to_response(
    inv: invoice_entity::Model,
    lines: Vec<line_entity::Model>,
) -> InvoiceResponse {
    InvoiceResponse {
        id: inv.id,
        invoice_no: inv.invoice_no,
        counterparty: inv.counterparty,
        issue_date: inv.issue_date.to_string(),
        tax_rate: inv.tax_rate.to_string(),
        subtotal: inv.subtotal.to_string(),
        tax: inv.tax.to_string(),
        discount_type: inv.discount_type,
        discount_value: inv.discount_value.map(|v| v.to_string()),
        discount_amount: inv.discount_amount.to_string(),
        total: inv.total.to_string(),
        status: status_str(&inv.status).to_owned(),
        created_by: inv.created_by,
        created_at: inv.created_at.to_rfc3339(),
        updated_at: inv.updated_at.to_rfc3339(),
        lines: lines.iter().map(line_to_response).collect(),
    }
}

// ── Calculation engine ────────────────────────────────────────────────────────

/// Compute the `line_total` for a new line request.
///
/// Formula:
/// ```text
/// base  = quantity × unit_price
/// adj   = percentage ? base × (value/100) : value
/// total = base − adj (discount)  |  base + adj (surcharge)
/// total = max(0, total)
/// ```
fn compute_line_total(
    quantity: Decimal,
    unit_price: Decimal,
    adj_type: Option<&AdjustmentType>,
    adj_is_pct: bool,
    adj_value: Option<Decimal>,
) -> AppResult<Decimal> {
    let base = (quantity * unit_price).round_dp(MONEY_DP);

    let Some(adj_type) = adj_type else {
        return Ok(base);
    };
    let adj_value = adj_value.unwrap_or(Decimal::ZERO);

    let adj_amount = if adj_is_pct {
        (base * (adj_value / Decimal::ONE_HUNDRED)).round_dp(MONEY_DP)
    } else {
        adj_value.round_dp(MONEY_DP)
    };

    let total = match adj_type {
        AdjustmentType::Discount => base - adj_amount,
        AdjustmentType::Surcharge => base + adj_amount,
    };

    Ok(total.max(Decimal::ZERO))
}

/// Recompute `subtotal`, `tax`, `discount_amount`, and `total` for an invoice
/// from its current line items and stored discount settings.
///
/// All amounts are rounded to 4 decimal places.
async fn recompute_invoice_totals(
    txn: &impl sea_orm::ConnectionTrait,
    invoice_id: i64,
    tax_rate: Decimal,
    discount_type: Option<&str>,
    discount_value: Option<Decimal>,
) -> Result<(Decimal, Decimal, Decimal, Decimal), sea_orm::DbErr> {
    let lines = line_entity::Entity::find()
        .filter(line_entity::Column::InvoiceId.eq(invoice_id))
        .all(txn)
        .await?;

    let subtotal = lines
        .iter()
        .fold(Decimal::ZERO, |acc, l| acc + l.line_total)
        .round_dp(MONEY_DP);

    let tax = (subtotal * tax_rate).round_dp(MONEY_DP);

    // Invoice-level discount.
    let discount_amount = match (discount_type, discount_value) {
        (Some("percentage"), Some(pct)) => {
            let raw = (subtotal * (pct / Decimal::ONE_HUNDRED)).round_dp(MONEY_DP);
            raw.min(MAX_DISCOUNT_AMOUNT)
        }
        (Some("fixed_amount"), Some(amt)) => amt.round_dp(MONEY_DP).min(MAX_DISCOUNT_AMOUNT),
        _ => Decimal::ZERO,
    };

    let total = (subtotal + tax - discount_amount).max(Decimal::ZERO);

    Ok((subtotal, tax, discount_amount, total))
}

// ══════════════════════════════════════════════════════════════════════════════
// Create invoice
// ══════════════════════════════════════════════════════════════════════════════

pub async fn create_invoice(
    conn: &DatabaseConnection,
    user_id: i64,
    req: CreateInvoiceRequest,
) -> AppResult<InvoiceResponse> {
    if req.invoice_no.trim().is_empty() {
        return Err(AppError::BadRequest("invoice_no is required".into()));
    }
    if req.counterparty.trim().is_empty() {
        return Err(AppError::BadRequest("counterparty is required".into()));
    }

    let issue_date = parse_date(&req.issue_date)?;
    let tax_rate = dec(req.tax_rate)?;

    if tax_rate < Decimal::ZERO || tax_rate > Decimal::ONE {
        return Err(AppError::BadRequest(
            "tax_rate must be in [0, 1] (e.g. 0.10 for 10 %)".into(),
        ));
    }

    let now = Utc::now();

    let model = InvoiceActiveModel {
        invoice_no: Set(req.invoice_no.trim().to_owned()),
        counterparty: Set(req.counterparty.trim().to_owned()),
        issue_date: Set(issue_date),
        tax_rate: Set(tax_rate),
        subtotal: Set(Decimal::ZERO),
        tax: Set(Decimal::ZERO),
        discount_type: Set(None),
        discount_value: Set(None),
        discount_amount: Set(Decimal::ZERO),
        total: Set(Decimal::ZERO),
        status: Set(InvoiceStatus::Draft),
        created_by: Set(user_id),
        created_at: Set(now),
        updated_at: Set(now),
        ..Default::default()
    }
    .insert(conn)
    .await
    .map_err(|e| {
        let msg = e.to_string();
        if msg.contains("UNIQUE") {
            AppError::Conflict(format!(
                "Invoice number '{}' already exists",
                req.invoice_no
            ))
        } else {
            AppError::Internal(msg)
        }
    })?;

    tracing::info!(
        invoice_id = model.id,
        invoice_no = %model.invoice_no,
        user_id,
        "billing.invoice_created"
    );
    let resp = invoice_to_response(model, vec![]);
    audit::service::append(
        conn,
        user_id,
        "invoice.created",
        "invoice",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({}),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Read
// ══════════════════════════════════════════════════════════════════════════════

async fn load_lines(
    conn: &DatabaseConnection,
    invoice_id: i64,
) -> AppResult<Vec<line_entity::Model>> {
    line_entity::Entity::find()
        .filter(line_entity::Column::InvoiceId.eq(invoice_id))
        .order_by_asc(line_entity::Column::CreatedAt)
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))
}

pub async fn get_invoice(conn: &DatabaseConnection, invoice_id: i64) -> AppResult<InvoiceResponse> {
    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    let lines = load_lines(conn, invoice_id).await?;
    Ok(invoice_to_response(inv, lines))
}

pub async fn list_invoices(
    conn: &DatabaseConnection,
    filter: InvoiceFilterQuery,
) -> AppResult<Vec<InvoiceResponse>> {
    let mut query = invoice_entity::Entity::find().order_by_desc(invoice_entity::Column::CreatedAt);

    if let Some(ref st) = filter.status {
        // Validate status string before filtering.
        let valid = ["draft", "issued", "paid", "cancelled", "overdue"];
        if !valid.contains(&st.as_str()) {
            return Err(AppError::BadRequest(format!(
                "Unknown status '{st}'. Valid: {}",
                valid.join(", ")
            )));
        }
        query = query.filter(invoice_entity::Column::Status.eq(st.as_str()));
    }
    if let Some(ref cp) = filter.counterparty {
        query = query.filter(invoice_entity::Column::Counterparty.eq(cp.as_str()));
    }

    let invoices = query
        .all(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    // Load lines for each invoice (N+1 is acceptable for small invoice volumes).
    let mut responses = Vec::with_capacity(invoices.len());
    for inv in invoices {
        let lines = load_lines(conn, inv.id).await?;
        responses.push(invoice_to_response(inv, lines));
    }
    Ok(responses)
}

// ══════════════════════════════════════════════════════════════════════════════
// Add line item
// ══════════════════════════════════════════════════════════════════════════════

pub async fn add_line(
    conn: &DatabaseConnection,
    invoice_id: i64,
    user_id: i64,
    req: AddLineRequest,
) -> AppResult<InvoiceResponse> {
    // ── Input validation ──────────────────────────────────────────────────────

    if req.description.trim().is_empty() {
        return Err(AppError::BadRequest("description is required".into()));
    }

    let pricing_model = parse_pricing_model(&req.pricing_model)?;

    if req.quantity <= 0.0 || !req.quantity.is_finite() {
        return Err(AppError::BadRequest(
            "quantity must be a positive finite number".into(),
        ));
    }
    if req.unit_price < 0.0 || !req.unit_price.is_finite() {
        return Err(AppError::BadRequest(
            "unit_price must be a non-negative finite number".into(),
        ));
    }

    let quantity = dec(req.quantity)?;
    let unit_price = dec(req.unit_price)?;

    let adj_type = req
        .adjustment_type
        .as_deref()
        .map(parse_adjustment_type)
        .transpose()?;

    let adj_value = req.adjustment_value.map(dec).transpose()?;

    // Validate adjustment constraints.
    if let Some(AdjustmentType::Discount) = &adj_type {
        let val = adj_value.unwrap_or(Decimal::ZERO);
        if req.adjustment_is_percentage {
            if val < Decimal::ZERO || val > MAX_DISCOUNT_PCT {
                return Err(AppError::UnprocessableEntity(format!(
                    "Percentage discount must be in [0, 30]; got {val}"
                )));
            }
        } else if val < Decimal::ZERO {
            return Err(AppError::UnprocessableEntity(
                "Fixed discount amount must be ≥ 0".into(),
            ));
        }
    }
    if let Some(AdjustmentType::Surcharge) = &adj_type {
        if adj_value.unwrap_or(Decimal::ZERO) < Decimal::ZERO {
            return Err(AppError::UnprocessableEntity(
                "Surcharge value must be ≥ 0".into(),
            ));
        }
    }

    let line_total = compute_line_total(
        quantity,
        unit_price,
        adj_type.as_ref(),
        req.adjustment_is_percentage,
        adj_value,
    )?;

    // ── Load invoice (must be draft) ──────────────────────────────────────────

    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    if inv.status != InvoiceStatus::Draft {
        return Err(AppError::Conflict(format!(
            "Invoice {invoice_id} is '{}' — line items can only be added to draft invoices",
            status_str(&inv.status)
        )));
    }

    // ── Insert line + recompute totals atomically ─────────────────────────────

    let now = Utc::now();
    let tax_rate = inv.tax_rate;
    let discount_type = inv.discount_type.clone();
    let discount_value_stored = inv.discount_value;

    let updated_inv = conn
        .transaction::<_, invoice_entity::Model, AppError>(|txn| {
            let adj_type = adj_type.clone();
            let adj_value = adj_value;
            let req = req.clone();
            let discount_type = discount_type.clone();
            Box::pin(async move {
                LineActiveModel {
                    invoice_id: Set(invoice_id),
                    description: Set(req.description.trim().to_owned()),
                    pricing_model: Set(pricing_model),
                    quantity: Set(req.quantity),
                    unit_price: Set(unit_price),
                    adjustment_type: Set(adj_type),
                    adjustment_is_percentage: Set(req.adjustment_is_percentage),
                    adjustment_value: Set(adj_value),
                    line_total: Set(line_total),
                    created_at: Set(now),
                    ..Default::default()
                }
                .insert(txn)
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

                let (subtotal, tax, disc_amount, total) = recompute_invoice_totals(
                    txn,
                    invoice_id,
                    tax_rate,
                    discount_type.as_deref(),
                    discount_value_stored,
                )
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

                let mut active: InvoiceActiveModel = inv.into();
                active.subtotal = Set(subtotal);
                active.tax = Set(tax);
                active.discount_amount = Set(disc_amount);
                active.total = Set(total);
                active.updated_at = Set(now);

                active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        invoice_id,
        user_id,
        line_total = %line_total,
        new_total  = %updated_inv.total,
        "billing.line_added"
    );

    let lines = load_lines(conn, invoice_id).await?;
    let resp = invoice_to_response(updated_inv, lines);
    audit::service::append(
        conn,
        user_id,
        "invoice.line_added",
        "invoice",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({ "line_total": line_total.to_string() }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Apply discount
// ══════════════════════════════════════════════════════════════════════════════

pub async fn apply_discount(
    conn: &DatabaseConnection,
    invoice_id: i64,
    user_id: i64,
    req: ApplyDiscountRequest,
) -> AppResult<InvoiceResponse> {
    // Validate discount type.
    if !matches!(req.discount_type.as_str(), "percentage" | "fixed_amount") {
        return Err(AppError::BadRequest(
            "discount_type must be 'percentage' or 'fixed_amount'".into(),
        ));
    }

    let discount_value = dec(req.discount_value)?;

    if req.discount_type == "percentage" {
        if discount_value < Decimal::ZERO || discount_value > MAX_DISCOUNT_PCT {
            return Err(AppError::UnprocessableEntity(format!(
                "Percentage discount must be in [0, 30]; got {discount_value}"
            )));
        }
    } else if discount_value < Decimal::ZERO {
        return Err(AppError::UnprocessableEntity(
            "Fixed discount amount must be ≥ 0".into(),
        ));
    }

    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    if inv.status != InvoiceStatus::Draft {
        return Err(AppError::Conflict(format!(
            "Invoice {invoice_id} is '{}' — discounts can only be applied to draft invoices",
            status_str(&inv.status)
        )));
    }

    let now = Utc::now();
    let tax_rate = inv.tax_rate;
    let dtype = req.discount_type.clone();

    let updated_inv = conn
        .transaction::<_, invoice_entity::Model, AppError>(|txn| {
            let dtype = dtype.clone();
            let inv_c = inv.clone();
            Box::pin(async move {
                let (subtotal, tax, disc_amount, total) = recompute_invoice_totals(
                    txn,
                    invoice_id,
                    tax_rate,
                    Some(dtype.as_str()),
                    Some(discount_value),
                )
                .await
                .map_err(|e| AppError::Internal(e.to_string()))?;

                let mut active: InvoiceActiveModel = inv_c.into();
                active.discount_type = Set(Some(dtype));
                active.discount_value = Set(Some(discount_value));
                active.discount_amount = Set(disc_amount);
                active.subtotal = Set(subtotal);
                active.tax = Set(tax);
                active.total = Set(total);
                active.updated_at = Set(now);

                active
                    .update(txn)
                    .await
                    .map_err(|e| AppError::Internal(e.to_string()))
            })
        })
        .await
        .map_err(tx_err)?;

    tracing::info!(
        invoice_id,
        user_id,
        discount_type = %req.discount_type,
        discount_value = %discount_value,
        discount_amount = %updated_inv.discount_amount,
        new_total = %updated_inv.total,
        "billing.discount_applied"
    );

    let lines = load_lines(conn, invoice_id).await?;
    let resp = invoice_to_response(updated_inv, lines);
    audit::service::append(
        conn,
        user_id,
        "invoice.discount_applied",
        "invoice",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({
            "discount_type":   req.discount_type,
            "discount_value":  discount_value.to_string(),
        }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Issue invoice (draft → issued)
// ══════════════════════════════════════════════════════════════════════════════

pub async fn issue_invoice(
    conn: &DatabaseConnection,
    invoice_id: i64,
    user_id: i64,
) -> AppResult<InvoiceResponse> {
    let inv = invoice_entity::Entity::find_by_id(invoice_id)
        .one(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
        .ok_or_else(|| AppError::NotFound(format!("Invoice {invoice_id} not found")))?;

    if inv.status != InvoiceStatus::Draft {
        return Err(AppError::Conflict(format!(
            "Invoice {invoice_id} is already '{}'; only draft invoices can be issued",
            status_str(&inv.status)
        )));
    }

    if inv.subtotal == Decimal::ZERO {
        return Err(AppError::UnprocessableEntity(
            "Cannot issue an invoice with no line items".into(),
        ));
    }

    let now = Utc::now();
    let mut active: InvoiceActiveModel = inv.into();
    active.status = Set(InvoiceStatus::Issued);
    active.updated_at = Set(now);

    let updated = active
        .update(conn)
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?;

    tracing::info!(
        invoice_id,
        user_id,
        total = %updated.total,
        "billing.invoice_issued"
    );

    let lines = load_lines(conn, invoice_id).await?;
    let resp = invoice_to_response(updated, lines);
    audit::service::append(
        conn,
        user_id,
        "invoice.issued",
        "invoice",
        resp.id,
        serde_json::to_value(&resp).unwrap_or_default(),
        serde_json::json!({ "previous_status": "draft" }),
    )
    .await?;
    Ok(resp)
}

// ══════════════════════════════════════════════════════════════════════════════
// Native Rust unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    // ── parse_pricing_model ─────────────────────────────────────────────────

    #[test]
    fn parse_pricing_model_all_valid() {
        assert_eq!(parse_pricing_model("fixed").unwrap(),        PricingModel::Fixed);
        assert_eq!(parse_pricing_model("per_unit").unwrap(),     PricingModel::PerUnit);
        assert_eq!(parse_pricing_model("percentage").unwrap(),   PricingModel::Percentage);
        assert_eq!(parse_pricing_model("per_duration").unwrap(), PricingModel::PerDuration);
        assert_eq!(parse_pricing_model("package").unwrap(),      PricingModel::Package);
    }

    #[test]
    fn parse_pricing_model_rejects_unknown() {
        assert!(parse_pricing_model("hourly").is_err());
        assert!(parse_pricing_model("").is_err());
    }

    #[test]
    fn pricing_model_str_roundtrips() {
        let variants = [
            PricingModel::Fixed, PricingModel::PerUnit, PricingModel::Percentage,
            PricingModel::PerDuration, PricingModel::Package,
        ];
        for v in variants {
            assert_eq!(parse_pricing_model(pricing_model_str(&v)).unwrap(), v);
        }
    }

    // ── parse_adjustment_type ───────────────────────────────────────────────

    #[test]
    fn parse_adjustment_type_valid() {
        assert_eq!(parse_adjustment_type("discount").unwrap(),  AdjustmentType::Discount);
        assert_eq!(parse_adjustment_type("surcharge").unwrap(), AdjustmentType::Surcharge);
    }

    #[test]
    fn parse_adjustment_type_rejects_unknown() {
        assert!(parse_adjustment_type("rebate").is_err());
    }

    // ── compute_line_total ──────────────────────────────────────────────────

    #[test]
    fn line_total_no_adjustment() {
        let total = compute_line_total(
            dec!(2.0), dec!(100.0), None, false, None,
        ).unwrap();
        assert_eq!(total, dec!(200.0));
    }

    #[test]
    fn line_total_fixed_discount() {
        let total = compute_line_total(
            dec!(1.0), dec!(500.0),
            Some(&AdjustmentType::Discount), false, Some(dec!(50.0)),
        ).unwrap();
        assert_eq!(total, dec!(450.0));
    }

    #[test]
    fn line_total_percentage_discount() {
        let total = compute_line_total(
            dec!(1.0), dec!(1000.0),
            Some(&AdjustmentType::Discount), true, Some(dec!(10.0)),
        ).unwrap();
        assert_eq!(total, dec!(900.0));
    }

    #[test]
    fn line_total_surcharge() {
        let total = compute_line_total(
            dec!(1.0), dec!(200.0),
            Some(&AdjustmentType::Surcharge), false, Some(dec!(25.0)),
        ).unwrap();
        assert_eq!(total, dec!(225.0));
    }

    #[test]
    fn line_total_discount_never_goes_negative() {
        let total = compute_line_total(
            dec!(1.0), dec!(10.0),
            Some(&AdjustmentType::Discount), false, Some(dec!(999.0)),
        ).unwrap();
        assert_eq!(total, dec!(0.0));
    }

    #[test]
    fn line_total_percentage_surcharge() {
        let total = compute_line_total(
            dec!(2.0), dec!(100.0),
            Some(&AdjustmentType::Surcharge), true, Some(dec!(20.0)),
        ).unwrap();
        // base=200, surcharge=200*20/100=40 → 240
        assert_eq!(total, dec!(240.0));
    }

    // ── status_str / parse_date / dec ───────────────────────────────────────

    #[test]
    fn status_str_covers_all_invoice_statuses() {
        assert_eq!(status_str(&InvoiceStatus::Draft),     "draft");
        assert_eq!(status_str(&InvoiceStatus::Issued),    "issued");
        assert_eq!(status_str(&InvoiceStatus::Paid),      "paid");
        assert_eq!(status_str(&InvoiceStatus::Cancelled), "cancelled");
        assert_eq!(status_str(&InvoiceStatus::Overdue),   "overdue");
    }

    #[test]
    fn parse_date_valid() {
        let d = parse_date("2026-06-15").unwrap();
        assert_eq!(d, NaiveDate::from_ymd_opt(2026, 6, 15).unwrap());
    }

    #[test]
    fn parse_date_invalid_rejects() {
        assert!(parse_date("not-a-date").is_err());
        assert!(parse_date("2026-13-01").is_err());
    }

    #[test]
    fn dec_helper_converts_f64() {
        let d = dec(42.5).unwrap();
        assert_eq!(d, Decimal::from_str_exact("42.5").unwrap());
    }
}
