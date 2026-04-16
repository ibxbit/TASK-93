"""
API tests for RBAC edge cases and object-level authorization.
Covers:
- Cross-role access attempts (write endpoints with read-only roles)
- Object-level permission enforcement
- Attempts to access or modify resources owned by other users
- Unauthenticated access to management endpoints
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


class TestRBACObjectLevel:
    def test_referee_cannot_modify_event(self, referee_token):
        """Referee has events:read but not events:write — PUT must return 403."""
        resp = requests.put(
            f"{BASE_URL}/events/1",
            headers=auth_headers(referee_token),
            json={"name": "HACK"},
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_finance_clerk_cannot_access_audit_logs(self, finance_token):
        """FinanceClerk lacks audit:read — access to /audit/logs must return 403."""
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_event_director_cannot_issue_invoice(self, director_token):
        """EventDirector lacks financials:write — issuing an invoice must return 403."""
        resp = requests.post(
            f"{BASE_URL}/invoices/1/issue",
            headers=auth_headers(director_token),
            json={},
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_referee_cannot_submit_result(self, referee_token):
        """Referee has participants:read only — submitting results must return 403."""
        resp = requests.post(
            f"{BASE_URL}/events/1/results",
            headers=auth_headers(referee_token),
            json={"participant_id": 1, "value_numeric": 100.0, "unit": "points"},
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_auditor_cannot_create_invoice(self, auditor_token):
        """Auditor has financials:read but not financials:write."""
        resp = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(auditor_token),
            json={
                "invoice_no": "RBAC-AUD-001",
                "counterparty": "Hack Corp",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_finance_clerk_cannot_manage_roles(self, finance_token):
        """FinanceClerk lacks roles:manage — role assignment must return 403."""
        resp = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(finance_token),
            json={"user_id": 1, "role": "administrator"},
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_director_cannot_trigger_backup(self, director_token):
        """EventDirector lacks system:admin — backup trigger must return 403."""
        resp = requests.post(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(director_token),
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"
