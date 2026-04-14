"""
API tests for RBAC edge cases and object-level authorization.
Covers:
- Cross-role access attempts
- Object-level permission enforcement
- Attempts to access or modify resources owned by other users
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers

class TestRBACObjectLevel:
    def test_referee_cannot_modify_event(self, referee_token):
        # Should be forbidden to update events
        resp = requests.put(f"{BASE_URL}/events/1", headers=auth_headers(referee_token), json={"name": "HACK"}, timeout=10)
        assert resp.status_code == 403

    def test_finance_clerk_cannot_access_audit_logs(self, finance_token):
        resp = requests.get(f"{BASE_URL}/audit/logs", headers=auth_headers(finance_token), timeout=10)
        assert resp.status_code == 403

    def test_event_director_cannot_issue_invoice(self, director_token):
        resp = requests.post(f"{BASE_URL}/invoices/1/issue", headers=auth_headers(director_token), timeout=10)
        assert resp.status_code == 403 or resp.status_code == 404
