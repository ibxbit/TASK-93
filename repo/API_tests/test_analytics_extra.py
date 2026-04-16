"""
API tests for advanced analytics endpoints: trend, funnel, retention.
Covers:
- GET /analytics/trends     (time-series; audit:read)
- GET /analytics/funnel     (conversion funnel; audit:read)
- GET /analytics/retention  (cohort retention; audit:read)
- RBAC enforcement
- Edge cases: empty data, extreme date ranges
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


class TestAnalyticsEndpoints:
    def test_trends_admin(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_revenue",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["metric"] == "invoice_revenue"
        assert "data" in body
        assert isinstance(body["data"], list)
        assert "bucket_size" in body

    def test_funnel_admin(self, admin_token):
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

    def test_retention_admin(self, admin_token):
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

    def test_trends_far_future_returns_empty_data(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_revenue"
            f"&start_date=9999-01-01&end_date=9999-01-02",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body["data"], list)
        # Far-future range should return zero or minimal data points
        assert len(body["data"]) <= 1

    def test_trends_rbac_referee_forbidden(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_revenue",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"
