//! Core domain entities — schema layer only, no business logic.
//!
//! All enum column types are defined in `enums` and enforce the same valid
//! value sets as the SQL CHECK constraints in the corresponding migrations.

pub mod enums;

// Event operations
pub mod event;
pub mod event_asset_binding;
pub mod result;
pub mod result_arbitration;
pub mod result_correction;
pub mod result_review;
pub mod ruleset_version;

// Asset management
pub mod asset;
pub mod asset_audit_log;
pub mod vehicle;
pub mod vehicle_audit_log;

// Financial settlement
pub mod invoice;
pub mod invoice_line;
pub mod payment_entry;
pub mod payment_exception;
pub mod payment_refund;

// Analytics — metric catalog
pub mod metric_definition;
pub mod metric_definition_version;

// Data quality
pub mod dq_scan_result;

// Unified audit trail
pub mod audit_log;
