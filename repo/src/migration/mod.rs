use sea_orm_migration::prelude::*;

mod m20240001_create_users;
mod m20240002_create_sessions;
mod m20240003_create_roles;
mod m20240004_create_role_assignments;
mod m20240005_create_ruleset_versions;
mod m20240006_create_events;
mod m20240007_create_results;
mod m20240008_create_result_reviews;
mod m20240009_create_assets;
mod m20240010_create_vehicles;
mod m20240011_create_invoices;
mod m20240012_create_invoice_lines;
mod m20240013_create_payment_entries;
mod m20240014_add_event_venue;
mod m20240015_create_event_asset_bindings;
mod m20240016_expand_result_unit_enum;
mod m20240017_add_event_championship_class;
mod m20240018_create_result_arbitrations;
mod m20240019_create_result_corrections;
mod m20240020_expand_assets;
mod m20240021_create_asset_audit_log;
mod m20240022_rebuild_vehicles;
mod m20240023_create_vehicle_audit_log;
mod m20240024_expand_invoice_lines;
mod m20240025_add_invoice_discount_tax_rate;
mod m20240026_expand_payment_methods;
mod m20240027_create_payment_exceptions;
mod m20240028_create_payment_refunds;
mod m20240029_add_invoice_line_refund;
mod m20240030_create_metric_definitions;
mod m20240031_create_metric_definition_versions;
mod m20240032_create_dq_scan_results;
mod m20240033_encrypt_payment_references;
mod m20240034_encrypt_personal_identifiers;
mod m20240035_create_audit_log;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            // Core identity & access
            Box::new(m20240001_create_users::Migration),
            Box::new(m20240002_create_sessions::Migration),
            Box::new(m20240003_create_roles::Migration),
            Box::new(m20240004_create_role_assignments::Migration),
            // Event operations
            Box::new(m20240005_create_ruleset_versions::Migration),
            Box::new(m20240006_create_events::Migration),
            Box::new(m20240007_create_results::Migration),
            Box::new(m20240008_create_result_reviews::Migration),
            // Asset management
            Box::new(m20240009_create_assets::Migration),
            Box::new(m20240010_create_vehicles::Migration),
            // Financial settlement
            Box::new(m20240011_create_invoices::Migration),
            Box::new(m20240012_create_invoice_lines::Migration),
            Box::new(m20240013_create_payment_entries::Migration),
            // Competition configuration — venue + equipment binding
            Box::new(m20240014_add_event_venue::Migration),
            Box::new(m20240015_create_event_asset_bindings::Migration),
            // Results — expanded unit enum (milliseconds, feet, inches)
            Box::new(m20240016_expand_result_unit_enum::Migration),
            // Review & arbitration support
            Box::new(m20240017_add_event_championship_class::Migration),
            Box::new(m20240018_create_result_arbitrations::Migration),
            Box::new(m20240019_create_result_corrections::Migration),
            // Asset ledger — full fields + audit log
            Box::new(m20240020_expand_assets::Migration),
            Box::new(m20240021_create_asset_audit_log::Migration),
            // Vehicle lifecycle
            Box::new(m20240022_rebuild_vehicles::Migration),
            Box::new(m20240023_create_vehicle_audit_log::Migration),
            // Billing — expanded pricing models + invoice discounts
            Box::new(m20240024_expand_invoice_lines::Migration),
            Box::new(m20240025_add_invoice_discount_tax_rate::Migration),
            // Payment methods expansion + exception / refund workflows
            Box::new(m20240026_expand_payment_methods::Migration),
            Box::new(m20240027_create_payment_exceptions::Migration),
            Box::new(m20240028_create_payment_refunds::Migration),
            // Invoice lines — refund flag
            Box::new(m20240029_add_invoice_line_refund::Migration),
            // Analytics — metric catalog
            Box::new(m20240030_create_metric_definitions::Migration),
            Box::new(m20240031_create_metric_definition_versions::Migration),
            // Data quality
            Box::new(m20240032_create_dq_scan_results::Migration),
            // Security — field-level encryption blind indexes
            Box::new(m20240033_encrypt_payment_references::Migration),
            Box::new(m20240034_encrypt_personal_identifiers::Migration),
            // Audit logging — unified append-only trail
            Box::new(m20240035_create_audit_log::Migration),
        ]
    }
}
