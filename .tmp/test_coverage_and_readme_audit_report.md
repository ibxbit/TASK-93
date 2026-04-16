# Test Coverage Audit

## Scope and Method
- Static inspection only. No code/test/script/container execution.
- Evidence sources: `repo/src/main.rs`, `repo/src/*/handlers.rs`, `repo/API_tests/*.py`, `repo/unit_tests/*.py`, `repo/conftest.py`, `repo/run_tests.sh`, `repo/docker-compose.yml`, `repo/README.md`.
- Project type declaration at README top: **backend** (`repo/README.md:3` = `Project Type: backend`).

## Backend Endpoint Inventory
Resolved from Rocket route macros and `mount` registrations.

1. `GET /health`
2. `POST /auth/login`
3. `POST /auth/logout`
4. `POST /auth/rotate-password`
5. `POST /admin/roles/assign`
6. `POST /admin/roles/revoke`
7. `GET /admin/users/:user_id/roles`
8. `POST /events`
9. `GET /events/:id`
10. `GET /events`
11. `PUT /events/:id`
12. `POST /events/:id/publish`
13. `POST /rulesets`
14. `GET /rulesets/:id`
15. `GET /rulesets`
16. `POST /rulesets/:id/rollback`
17. `POST /events/:event_id/results`
18. `GET /events/:event_id/rankings`
19. `POST /events/:event_id/results/:result_id/reviews`
20. `GET /events/:event_id/results/:result_id/reviews`
21. `POST /events/:event_id/results/:result_id/arbitrate`
22. `POST /events/:event_id/results/:result_id/corrections`
23. `GET /events/:event_id/results/:result_id/corrections`
24. `POST /events/:event_id/results/:result_id/corrections/:correction_id/resolve`
25. `GET /events/:event_id/results/export`
26. `POST /vehicles`
27. `GET /vehicles/:id`
28. `GET /vehicles`
29. `PUT /vehicles/:id`
30. `POST /vehicles/:id/status`
31. `GET /vehicles/:id/history`
32. `POST /assets`
33. `GET /assets/:id`
34. `GET /assets`
35. `PUT /assets/:id`
36. `PATCH /assets/:id/status`
37. `GET /assets/:id/history`
38. `GET /assets/export`
39. `POST /assets/import`
40. `POST /invoices`
41. `GET /invoices/:id`
42. `GET /invoices`
43. `POST /invoices/:id/lines`
44. `POST /invoices/:id/discount`
45. `POST /invoices/:id/issue`
46. `POST /invoices/:invoice_id/payments`
47. `GET /invoices/:invoice_id/payments`
48. `POST /invoices/:invoice_id/payments/:payment_id/exceptions`
49. `GET /invoices/:invoice_id/payments/:payment_id/exceptions`
50. `POST /invoices/:invoice_id/payments/:payment_id/refunds`
51. `POST /invoices/:invoice_id/payments/:payment_id/refunds/:refund_id/approve`
52. `POST /invoices/:invoice_id/payments/:payment_id/refunds/:refund_id/reject`
53. `POST /metrics`
54. `GET /metrics`
55. `GET /metrics/:id`
56. `PUT /metrics/:id`
57. `GET /analytics/trends`
58. `GET /analytics/funnel`
59. `GET /analytics/retention`
60. `GET /analytics/export`
61. `POST /data-quality/scans`
62. `GET /data-quality/scans`
63. `GET /data-quality/scans/:id`
64. `GET /audit/logs`
65. `GET /audit/logs/:id`
66. `GET /admin/backups`
67. `POST /admin/backups`
68. `POST /admin/backups/:filename/restore`

## API Test Mapping Table

| Endpoint | covered | test type | test file(s) | evidence |
|---|---|---|---|---|
| `GET /health` | yes | true no-mock HTTP | `repo/API_tests/test_auth.py` | `TestHealthEndpoint.test_health_returns_ok` |
| `POST /auth/login` | yes | true no-mock HTTP | `repo/API_tests/test_auth.py` | `TestLogin.test_admin_login_success` |
| `POST /auth/logout` | yes | true no-mock HTTP | `repo/API_tests/test_auth.py` | `TestLogout.test_logout_with_valid_token_succeeds` |
| `POST /auth/rotate-password` | yes | true no-mock HTTP | `repo/API_tests/test_edge_cases.py` | `TestPasswordRotation.test_rotate_with_wrong_current_password_fails` |
| `POST /admin/roles/assign` | yes | true no-mock HTTP | `repo/API_tests/test_rbac.py` | `TestRoleAssignment.test_duplicate_role_assignment_returns_409` |
| `POST /admin/roles/revoke` | yes | true no-mock HTTP | `repo/API_tests/test_rbac.py` | `TestRoleAssignment.test_revoke_nonexistent_role_returns_404` |
| `GET /admin/users/:user_id/roles` | yes | true no-mock HTTP | `repo/API_tests/test_rbac.py` | `TestRoleAssignment.test_admin_can_list_user_roles` |
| `POST /events` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestEventCreate.test_create_event_returns_draft_status` |
| `GET /events/:id` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestEventRead.test_get_event_by_id` |
| `GET /events` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestEventRead.test_list_events_returns_array` |
| `PUT /events/:id` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestEventUpdate.test_update_draft_event` |
| `POST /events/:id/publish` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestEventPublish.test_publish_event_transitions_to_published` |
| `POST /rulesets` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestRulesetVersion.test_create_ruleset_returns_version` |
| `GET /rulesets/:id` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestRulesetVersion.test_get_ruleset_by_id` |
| `GET /rulesets` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestRulesetVersion.test_list_rulesets_returns_array` |
| `POST /rulesets/:id/rollback` | yes | true no-mock HTTP | `repo/API_tests/test_events.py` | `TestRulesetVersion.test_rollback_ruleset_creates_new_version` |
| `POST /events/:event_id/results` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestResultSubmission.test_submit_result_returns_200` |
| `GET /events/:event_id/rankings` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestRankingsAndAdvancement.test_rankings_returns_response_shape` |
| `POST /events/:event_id/results/:result_id/reviews` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestNonChampionshipReview.test_single_approval_auto_approves_result` |
| `GET /events/:event_id/results/:result_id/reviews` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestNonChampionshipReview.test_list_reviews_returns_array` |
| `POST /events/:event_id/results/:result_id/arbitrate` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestArbitration.test_director_can_arbitrate_result` |
| `POST /events/:event_id/results/:result_id/corrections` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestCorrectionsWorkflow.test_request_correction_returns_pending` |
| `GET /events/:event_id/results/:result_id/corrections` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestCorrectionsWorkflow.test_list_corrections_returns_array` |
| `POST /events/:event_id/results/:result_id/corrections/:correction_id/resolve` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestCorrectionsWorkflow.test_approve_correction_transitions_to_approved` |
| `GET /events/:event_id/results/export` | yes | true no-mock HTTP | `repo/API_tests/test_results.py` | `TestResultsExport.test_auditor_can_export_results` |
| `POST /vehicles` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleCreate.test_admin_can_create_vehicle` |
| `GET /vehicles/:id` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleRead.test_get_vehicle_by_id` |
| `GET /vehicles` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleRead.test_list_vehicles_returns_array` |
| `PUT /vehicles/:id` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleUpdate.test_update_vehicle_fields` |
| `POST /vehicles/:id/status` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleStatusTransition.test_transition_draft_to_published` |
| `GET /vehicles/:id/history` | yes | true no-mock HTTP | `repo/API_tests/test_vehicles.py` | `TestVehicleHistory.test_get_vehicle_history_returns_array` |
| `POST /assets` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetCreate.test_admin_can_create_asset` |
| `GET /assets/:id` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetRead.test_get_asset_by_id` |
| `GET /assets` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetRead.test_list_assets_returns_array` |
| `PUT /assets/:id` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetUpdate.test_update_asset_fields` |
| `PATCH /assets/:id/status` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetStatusUpdate.test_update_status_to_out_for_repair` |
| `GET /assets/:id/history` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetHistory.test_get_asset_history_returns_array` |
| `GET /assets/export` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetExportImport.test_export_returns_array` |
| `POST /assets/import` | yes | true no-mock HTTP | `repo/API_tests/test_assets.py` | `TestAssetExportImport.test_import_assets_bulk` |
| `POST /invoices` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestInvoiceCreate.test_finance_clerk_can_create_invoice` |
| `GET /invoices/:id` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestInvoiceRead.test_get_invoice_by_id` |
| `GET /invoices` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestInvoiceRead.test_list_invoices_returns_array` |
| `POST /invoices/:id/lines` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestLineItems.test_add_line_item` |
| `POST /invoices/:id/discount` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestDiscount.test_apply_percentage_discount` |
| `POST /invoices/:id/issue` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestIssueInvoice.test_issue_transitions_to_issued` |
| `POST /invoices/:invoice_id/payments` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestPayments.test_record_payment` |
| `GET /invoices/:invoice_id/payments` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestPayments.test_list_payments_for_invoice` |
| `POST /invoices/:invoice_id/payments/:payment_id/exceptions` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestPaymentExceptions.test_void_sets_payment_status_voided` |
| `GET /invoices/:invoice_id/payments/:payment_id/exceptions` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestPaymentExceptions.test_list_exceptions_returns_array` |
| `POST /invoices/:invoice_id/payments/:payment_id/refunds` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestRefundWorkflow.test_request_refund_starts_pending_finance` |
| `POST /invoices/:invoice_id/payments/:payment_id/refunds/:refund_id/approve` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestRefundWorkflow.test_small_refund_finance_approval_completes` |
| `POST /invoices/:invoice_id/payments/:payment_id/refunds/:refund_id/reject` | yes | true no-mock HTTP | `repo/API_tests/test_billing.py` | `TestRefundWorkflow.test_finance_clerk_can_reject_refund` |
| `POST /metrics` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestMetricCreate.test_finance_clerk_can_create_metric` |
| `GET /metrics` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestMetricRead.test_list_metrics_returns_array` |
| `GET /metrics/:id` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestMetricRead.test_get_metric_by_id` |
| `PUT /metrics/:id` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestMetricUpdate.test_update_metric_bumps_version` |
| `GET /analytics/trends` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestTrends.test_trends_returns_response_shape` |
| `GET /analytics/funnel` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestFunnel.test_invoice_lifecycle_funnel` |
| `GET /analytics/retention` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestRetention.test_event_participation_retention` |
| `GET /analytics/export` | yes | true no-mock HTTP | `repo/API_tests/test_analytics.py` | `TestExport.test_export_trends_csv` |
| `POST /data-quality/scans` | yes | true no-mock HTTP | `repo/API_tests/test_data_quality.py` | `TestDataQualityAccess.test_admin_can_run_scan` |
| `GET /data-quality/scans` | yes | true no-mock HTTP | `repo/API_tests/test_data_quality.py` | `TestScanList.test_list_scans_returns_array` |
| `GET /data-quality/scans/:id` | yes | true no-mock HTTP | `repo/API_tests/test_data_quality.py` | `TestScanGet.test_get_scan_by_id` |
| `GET /audit/logs` | yes | true no-mock HTTP | `repo/API_tests/test_audit.py` | `TestAuditLogAccess.test_auditor_can_list_audit_logs` |
| `GET /audit/logs/:id` | yes | true no-mock HTTP | `repo/API_tests/test_audit.py` | `TestAuditLogEntry.test_get_audit_log_entry_by_id` |
| `GET /admin/backups` | yes | true no-mock HTTP | `repo/API_tests/test_backup_restore.py` | `TestBackupList.test_list_response_shape` |
| `POST /admin/backups` | yes | true no-mock HTTP | `repo/API_tests/test_backup_restore.py` | `TestBackupCreate.test_trigger_backup_returns_ok_with_metadata` |
| `POST /admin/backups/:filename/restore` | yes | true no-mock HTTP | `repo/API_tests/test_backup_restore.py` | `TestRestoreStaged.test_restore_of_existing_backup_returns_pending_restart` |

## Coverage Summary
- total endpoints: **68**
- endpoints with HTTP tests: **68**
- endpoints with TRUE no-mock tests: **68**
- HTTP coverage: **100.0%**
- True API coverage: **100.0%**

## Unit Test Summary

### Backend Unit Tests
- test files: `repo/unit_tests/test_business_logic.py`, `repo/unit_tests/test_data_quality_logic.py`, `repo/unit_tests/test_encryption_logic.py`, `repo/unit_tests/test_rbac_logic.py`, `repo/unit_tests/test_results_logic.py`, `repo/unit_tests/test_ruleset_logic.py`, `repo/unit_tests/test_validation.py`
- modules covered:
  - controllers: no direct Rust controller unit tests
  - services: spec-level parity tests only (Python re-implementation)
  - repositories: no direct repository tests
  - auth/guards/middleware: spec-level mapping checks only
- important backend modules NOT tested directly: `repo/src/*/service.rs`, `repo/src/rbac/guards.rs`, `repo/src/middleware/*`, `repo/src/db.rs`, `repo/src/entity/*`.

### Frontend Unit Tests
- frontend test files: **NONE**
- frameworks/tools detected: **NONE**
- components/modules covered: **NONE**
- important frontend components/modules NOT tested: **N/A (backend-only repo)**
- **Frontend unit tests: MISSING**

### Cross-Layer Observation
- Backend-only project. README explicitly states no frontend (`repo/README.md:30`).

## Tests Check
- API test classification:
  1. True no-mock HTTP: all `repo/API_tests/*.py`
  2. HTTP with mocking: none detected
  3. Non-HTTP: all `repo/unit_tests/*.py`
- Mock detection evidence: no `jest.mock`, `vi.mock`, `sinon.stub`, `unittest.mock`, `monkeypatch` found in `repo/API_tests` and `repo/unit_tests`.
- API observability: mostly strong (method/path, inputs, outputs asserted), with some status-only RBAC assertions in `repo/API_tests/test_rbac.py`.
- quality/sufficiency:
  - success, failure, edge, validation, RBAC paths: broad
  - depth: mixed (some shallow status-only assertions)
- `run_tests.sh`: Docker-based (`repo/run_tests.sh:18`, `repo/docker-compose.yml:91-112`) → **OK**

## Test Coverage Score (0-100)
**84/100**

## Score Rationale
- High endpoint-level HTTP coverage and no over-mocking.
- Deductions for unit layer being Python spec duplication rather than direct Rust implementation tests.
- Deductions for uneven assertion depth in some tests.

## Key Gaps
1. Spec-vs-implementation gap for unit tests (Rust modules not directly unit-tested).
2. Some API tests assert status only.
3. Repository/query internals verified mostly indirectly.

## Confidence & Assumptions
- Confidence: high for endpoint mapping and HTTP coverage (direct static evidence).
- Assumption: no hidden conditional route registration outside observed mounts.

## Test Coverage Verdict
**PASS (with quality caveats)**

---

# README Audit

## README Location
- `repo/README.md` exists.

## Hard Gate Failures
- None.

## High Priority Issues
- None.

## Medium Priority Issues
- None.

## Low Priority Issues
- Minor redundancy remains: endpoint documentation appears in both the main API catalog and section-specific repeat blocks (maintenance overhead risk), e.g. `repo/README.md:66` and `repo/README.md:344`.

## README Verdict
**PASS**

## Evidence-Based Hard Gate Checks
- formatting: pass (structured markdown/tables/code blocks)
- startup command: pass (`docker-compose up` at `repo/README.md:12`)
- access method: pass (`http://localhost:8000` at `repo/README.md:27`)
- verification method: pass (curl workflow at `repo/README.md:173`)
- environment rules: pass (Docker-contained startup/tests, no runtime host installs required)
- demo credentials with roles: pass (`repo/README.md:38-44`)

---

## Final Combined Verdicts
- Test Coverage Audit: **PASS (with quality caveats)**
- README Audit: **PASS**
