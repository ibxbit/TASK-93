use sea_orm::entity::prelude::*;

// ── Event ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum EventStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "published")]
    Published,
    #[sea_orm(string_value = "in_progress")]
    InProgress,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

// ── Result ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ResultUnit {
    #[sea_orm(string_value = "milliseconds")]
    Milliseconds,
    #[sea_orm(string_value = "feet")]
    Feet,
    #[sea_orm(string_value = "inches")]
    Inches,
    #[sea_orm(string_value = "seconds")]
    Seconds,
    #[sea_orm(string_value = "meters")]
    Meters,
    #[sea_orm(string_value = "kilometers")]
    Kilometers,
    #[sea_orm(string_value = "kilograms")]
    Kilograms,
    #[sea_orm(string_value = "points")]
    Points,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ReviewedState {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ReviewDecision {
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

// ── Correction ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum CorrectionStatus {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}

// ── Asset ─────────────────────────────────────────────────────────────────────

/// Operational lifecycle status of a physical asset.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum AssetStatus {
    #[sea_orm(string_value = "in_service")]
    InService,
    #[sea_orm(string_value = "out_for_repair")]
    OutForRepair,
    #[sea_orm(string_value = "retired")]
    Retired,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum AssetCategory {
    #[sea_orm(string_value = "vehicle")]
    Vehicle,
    #[sea_orm(string_value = "equipment")]
    Equipment,
    #[sea_orm(string_value = "facility")]
    Facility,
    #[sea_orm(string_value = "electronic")]
    Electronic,
    #[sea_orm(string_value = "other")]
    Other,
}

// ── Vehicle ───────────────────────────────────────────────────────────────────

/// Legacy operational status — preserved only for the original migration.
/// New code uses `VehicleLifecycleStatus`.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum VehicleStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "inactive")]
    Inactive,
    #[sea_orm(string_value = "retired")]
    Retired,
    #[sea_orm(string_value = "under_maintenance")]
    UnderMaintenance,
}

/// Commercial lifecycle state for a vehicle in the registry.
///
/// Allowed transitions (strictly enforced):
/// - `Draft`     → `Published`
/// - `Published` → `Delisted`  (reason required)
/// - `Published` → `Sold`      (reason required)
/// - `Delisted`  → `Published` (re-listing)
/// - `Delisted`  → `Sold`      (reason required)
/// - `Sold` is terminal — no further transitions.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum VehicleLifecycleStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "published")]
    Published,
    #[sea_orm(string_value = "delisted")]
    Delisted,
    #[sea_orm(string_value = "sold")]
    Sold,
}

// ── Invoice ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum InvoiceStatus {
    #[sea_orm(string_value = "draft")]
    Draft,
    #[sea_orm(string_value = "issued")]
    Issued,
    #[sea_orm(string_value = "paid")]
    Paid,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
    #[sea_orm(string_value = "overdue")]
    Overdue,
}

// ── Invoice line ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PricingModel {
    /// Flat fee independent of quantity.
    #[sea_orm(string_value = "fixed")]
    Fixed,
    /// Price per discrete use / occurrence.
    #[sea_orm(string_value = "per_unit")]
    PerUnit,
    /// Percentage-based pricing (line total = quantity × unit_price × pct).
    #[sea_orm(string_value = "percentage")]
    Percentage,
    /// Price per unit of time (hours, days, etc.).
    #[sea_orm(string_value = "per_duration")]
    PerDuration,
    /// Bundled package at a fixed price per package.
    #[sea_orm(string_value = "package")]
    Package,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum AdjustmentType {
    #[sea_orm(string_value = "discount")]
    Discount,
    #[sea_orm(string_value = "surcharge")]
    Surcharge,
}

// ── Payment ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PaymentMethod {
    #[sea_orm(string_value = "bank_transfer")]
    BankTransfer,
    #[sea_orm(string_value = "card")]
    Card,
    #[sea_orm(string_value = "cash")]
    Cash,
    #[sea_orm(string_value = "cheque")]
    Cheque,
    /// ACH (Automated Clearing House) electronic transfer — stores the ACH reference number.
    #[sea_orm(string_value = "ach")]
    Ach,
}

/// Lifecycle status of a single payment entry.
///
/// - `Active`   — payment is valid and counts toward the invoice balance.
/// - `Voided`   — cancelled before settlement; excluded from paid totals.
/// - `Reversed` — settled but subsequently reversed by the bank.
/// - `Disputed` — subject to a chargeback / dispute; excluded from paid totals.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum PaymentEntryStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "voided")]
    Voided,
    #[sea_orm(string_value = "reversed")]
    Reversed,
    #[sea_orm(string_value = "disputed")]
    Disputed,
}

/// The kind of exception being raised against a payment.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum ExceptionType {
    /// Payment cancelled before settlement.
    #[sea_orm(string_value = "void")]
    Void,
    /// Payment settled but then reversed by the bank (e.g. ACH return).
    #[sea_orm(string_value = "reversal")]
    Reversal,
    /// Payer initiated a chargeback or dispute.
    #[sea_orm(string_value = "dispute")]
    Dispute,
}

/// Approval workflow status for a refund request.
///
/// Flow:
/// - `PendingFinance`  — awaiting Finance Clerk sign-off.
/// - `PendingAuditor`  — Finance approved; awaiting Auditor sign-off (required for > $1,000).
/// - `Approved`        — all required approvals obtained; refund line inserted.
/// - `Rejected`        — denied at any stage.
#[derive(Debug, Clone, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum RefundStatus {
    #[sea_orm(string_value = "pending_finance")]
    PendingFinance,
    #[sea_orm(string_value = "pending_auditor")]
    PendingAuditor,
    #[sea_orm(string_value = "approved")]
    Approved,
    #[sea_orm(string_value = "rejected")]
    Rejected,
}
