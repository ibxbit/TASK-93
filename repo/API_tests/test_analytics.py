"""
API tests for the analytics module.

Covers:
- POST /metrics              (create metric definition; financials:write)
- GET  /metrics              (list metrics;            financials:read)
- GET  /metrics/<id>         (get metric + history;    financials:read)
- PUT  /metrics/<id>         (update metric / version; financials:write)
- GET  /analytics/trends     (time-series trend;       audit:read)
- GET  /analytics/funnel     (conversion funnel;       audit:read)
- GET  /analytics/retention  (cohort retention;        audit:read)
- GET  /analytics/export     (CSV download;            audit:read)
- RBAC enforcement for every endpoint
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Fixtures ──────────────────────────────────────────────────────────────────

@pytest.fixture(scope="module")
def metric(admin_token, ts):
    """Create a metric definition for this test module."""
    resp = requests.post(
        f"{BASE_URL}/metrics",
        headers=auth_headers(admin_token),
        json={
            "name": f"test_metric_{ts}",
            "definition": "Total invoice revenue for the period",
            "unit": "dollars",
            "category": "financial",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Metric creation failed: {resp.text}"
    return resp.json()


# ── Metric catalog ────────────────────────────────────────────────────────────

class TestMetricCreate:
    def test_finance_clerk_can_create_metric(self, finance_token, ts):
        resp = requests.post(
            f"{BASE_URL}/metrics",
            headers=auth_headers(finance_token),
            json={
                "name": f"fc_metric_{ts}",
                "definition": "Finance clerk metric",
                "unit": "count",
                "category": "financial",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["version"] == 1
        assert body["is_active"] is True

    def test_metric_has_required_fields(self, metric):
        required = {
            "id", "name", "definition", "unit", "category",
            "version", "owner_id", "is_active", "created_at", "updated_at",
        }
        assert required.issubset(metric.keys())

    def test_metric_starts_at_version_1(self, metric):
        assert metric["version"] == 1

    def test_metric_is_active_by_default(self, metric):
        assert metric["is_active"] is True

    def test_duplicate_metric_name_returns_conflict(self, admin_token, metric):
        resp = requests.post(
            f"{BASE_URL}/metrics",
            headers=auth_headers(admin_token),
            json={
                "name": metric["name"],  # same name
                "definition": "Duplicate attempt",
                "category": "financial",
            },
            timeout=10,
        )
        assert resp.status_code in (409, 422)

    def test_referee_cannot_create_metric(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/metrics",
            headers=auth_headers(referee_token),
            json={
                "name": f"ref_metric_{ts}",
                "definition": "Should be forbidden",
                "category": "operational",
            },
            timeout=10,
        )
        assert resp.status_code == 403

    def test_auditor_cannot_create_metric(self, auditor_token, ts):
        resp = requests.post(
            f"{BASE_URL}/metrics",
            headers=auth_headers(auditor_token),
            json={
                "name": f"aud_metric_{ts}",
                "definition": "Auditor read-only",
                "category": "operational",
            },
            timeout=10,
        )
        assert resp.status_code == 403

    def test_unauthenticated_create_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/metrics",
            json={"name": "x", "definition": "y", "category": "operational"},
            timeout=10,
        )
        assert resp.status_code == 401


class TestMetricRead:
    def test_list_metrics_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/metrics",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_finance_clerk_can_list_metrics(self, finance_token):
        resp = requests.get(
            f"{BASE_URL}/metrics",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_auditor_can_list_metrics(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/metrics",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_list_metrics(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/metrics",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_get_metric_by_id(self, admin_token, metric):
        resp = requests.get(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "metric" in body
        assert "history" in body
        assert body["metric"]["id"] == metric["id"]

    def test_metric_history_has_version_entries(self, admin_token, metric):
        resp = requests.get(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert isinstance(resp["history"], list)
        assert len(resp["history"]) >= 1
        assert resp["history"][0]["version"] == 1

    def test_get_nonexistent_metric_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/metrics/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404


class TestMetricUpdate:
    def test_update_metric_bumps_version(self, admin_token, metric):
        resp = requests.put(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            json={
                "definition": "Updated definition for testing",
                "unit": "dollars",
                "change_reason": "Refined calculation scope",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["version"] == metric["version"] + 1
        assert body["definition"] == "Updated definition for testing"

    def test_update_adds_history_entry(self, admin_token, metric):
        before = requests.get(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()["history"]
        before_count = len(before)

        requests.put(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            json={
                "definition": "Another update",
                "change_reason": "Second update",
            },
            timeout=10,
        )

        after = requests.get(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()["history"]
        assert len(after) > before_count

    def test_referee_cannot_update_metric(self, referee_token, metric):
        resp = requests.put(
            f"{BASE_URL}/metrics/{metric['id']}",
            headers=auth_headers(referee_token),
            json={"definition": "Forbidden", "change_reason": "x"},
            timeout=10,
        )
        assert resp.status_code == 403


# ── Analytics endpoints ───────────────────────────────────────────────────────

class TestTrends:
    def test_trends_returns_response_shape(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_revenue"
            f"&start_date=2024-01-01&end_date=2026-12-31",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "metric" in body
        assert "bucket_size" in body
        assert "start_date" in body
        assert "end_date" in body
        assert "data" in body
        assert isinstance(body["data"], list)

    def test_trends_invoice_count_metric(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["metric"] == "invoice_count"

    def test_trends_payment_volume_metric(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=payment_volume",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_trends_results_submitted_metric(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=results_submitted",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_trends_active_assets_metric(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=active_assets",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_trends_day_bucket(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count&bucket=day"
            f"&start_date=2026-01-01&end_date=2026-03-31",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["bucket_size"] == "day"

    def test_trends_week_bucket(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count&bucket=week"
            f"&start_date=2026-01-01&end_date=2026-03-31",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["bucket_size"] == "week"

    def test_trends_month_bucket_default(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["bucket_size"] == "month"

    def test_trends_unknown_metric_returns_400(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=nonexistent_kpi",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 400

    def test_trends_auditor_can_access(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_trends_referee_cannot_access(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_trends_finance_clerk_cannot_access(self, finance_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_trends_unauthenticated_returns_401(self):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count",
            timeout=10,
        )
        assert resp.status_code == 401

    def test_trends_data_points_have_bucket_and_value(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_revenue"
            f"&start_date=2024-01-01&end_date=2026-12-31",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        for point in resp["data"]:
            assert "bucket" in point
            assert "value" in point


class TestFunnel:
    def test_invoice_lifecycle_funnel(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invoice_lifecycle",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["funnel_type"] == "invoice_lifecycle"
        assert "steps" in body
        assert isinstance(body["steps"], list)

    def test_result_review_funnel(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=result_review",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["funnel_type"] == "result_review"

    def test_refund_approval_funnel(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=refund_approval",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_funnel_steps_have_required_fields(self, admin_token):
        steps = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invoice_lifecycle",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()["steps"]
        for step in steps:
            assert "stage" in step
            assert "count" in step
            assert "conversion_rate" in step
            assert "drop_off" in step

    def test_funnel_response_has_date_range(self, admin_token):
        body = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invoice_lifecycle"
            f"&start_date=2025-01-01&end_date=2026-12-31",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert "start_date" in body
        assert "end_date" in body

    def test_funnel_unknown_type_returns_400(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invalid_funnel",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 400

    def test_funnel_referee_cannot_access(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invoice_lifecycle",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_funnel_auditor_can_access(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/funnel?funnel_type=invoice_lifecycle",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200


class TestRetention:
    def test_event_participation_retention(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["retention_type"] == "event_participation"
        assert "rows" in body
        assert isinstance(body["rows"], list)

    def test_invoice_payment_retention(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=invoice_payment",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_retention_rows_have_required_fields(self, admin_token):
        rows = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()["rows"]
        for row in rows:
            assert "cohort" in row
            assert "cohort_size" in row
            assert "periods" in row
            assert isinstance(row["periods"], list)

    def test_retention_periods_have_count_and_rate(self, admin_token):
        rows = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()["rows"]
        for row in rows:
            for period in row["periods"]:
                assert "period" in period
                assert "count" in period
                assert "rate" in period

    def test_retention_custom_periods(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation&periods=2",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        rows = resp.json()["rows"]
        for row in rows:
            assert len(row["periods"]) <= 2

    def test_retention_unknown_type_returns_400(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=unknown",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 400

    def test_retention_referee_cannot_access(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_retention_auditor_can_access(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/retention?retention_type=event_participation",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200


class TestExport:
    def test_export_trends_csv(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_revenue",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_export_content_type_is_csv(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert "text/csv" in resp.headers.get("Content-Type", "")

    def test_export_has_content_disposition(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        disposition = resp.headers.get("Content-Disposition", "")
        assert "attachment" in disposition

    def test_export_funnel_csv(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export"
            f"?report_type=funnel&funnel_type=invoice_lifecycle",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_export_retention_csv(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export"
            f"?report_type=retention&retention_type=event_participation",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_export_csv_body_is_non_empty(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.text) > 0

    def test_export_csv_has_header_row(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        lines = [l for l in resp.text.strip().splitlines() if l]
        assert len(lines) >= 1  # at least a header row

    def test_export_unknown_report_type_returns_400(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=unknown_type",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 400

    def test_export_referee_cannot_access(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_export_auditor_can_access(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_export_unauthenticated_returns_401(self):
        resp = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends&metric=invoice_count",
            timeout=10,
        )
        assert resp.status_code == 401
