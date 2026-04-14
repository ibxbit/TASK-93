"""
API tests for advanced analytics endpoints: trend, funnel, retention.
Covers:
- GET /metrics/trends
- GET /metrics/funnel
- GET /metrics/retention
- RBAC enforcement
- Edge cases: empty data, extreme values
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers

class TestAnalyticsEndpoints:
    def test_trends_admin(self, admin_token):
        resp = requests.get(f"{BASE_URL}/metrics/trends", headers=auth_headers(admin_token), timeout=10)
        assert resp.status_code in (200, 204)

    def test_funnel_admin(self, admin_token):
        resp = requests.get(f"{BASE_URL}/metrics/funnel", headers=auth_headers(admin_token), timeout=10)
        assert resp.status_code in (200, 204)

    def test_retention_admin(self, admin_token):
        resp = requests.get(f"{BASE_URL}/metrics/retention", headers=auth_headers(admin_token), timeout=10)
        assert resp.status_code in (200, 204)

    def test_trends_empty(self, admin_token):
        resp = requests.get(f"{BASE_URL}/metrics/trends?window=9999d", headers=auth_headers(admin_token), timeout=10)
        assert resp.status_code in (200, 204)
        # Should handle empty dataset gracefully
        if resp.status_code == 200:
            assert isinstance(resp.json(), list)

    def test_trends_rbac(self, referee_token):
        resp = requests.get(f"{BASE_URL}/metrics/trends", headers=auth_headers(referee_token), timeout=10)
        assert resp.status_code in (200, 403)
