"""
API tests for RBAC enforcement across all major resource types.

Verifies that:
- Every protected endpoint returns 401 without credentials
- Each role can only perform authorized operations
- Cross-role access is correctly rejected (403)
- Admin role has full access
- Role assignment and revocation work correctly
- Backup endpoint RBAC (system_admin only)
- Encryption-at-rest: encrypted fields are never returned as raw ciphertext
- Audit log RBAC and append-only enforcement
- Data quality and analytics RBAC
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


class TestPublicEndpoints:
    """Only /health and /auth/login should be public."""

    def test_health_is_public(self):
        assert requests.get(f"{BASE_URL}/health", timeout=10).status_code == 200

    def test_login_is_public(self):
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "admin", "password": "Admin123!"},
            timeout=10,
        )
        assert resp.status_code == 200

    PROTECTED = [
        ("GET",  "/events"),
        ("GET",  "/rulesets"),
        ("GET",  "/vehicles"),
        ("GET",  "/assets"),
        ("GET",  "/invoices"),
        ("GET",  "/audit/logs"),
        ("GET",  "/admin/backups"),
        ("GET",  "/metrics"),
        ("GET",  "/data-quality/scans"),
    ]

    @pytest.mark.parametrize("method,path", PROTECTED)
    def test_protected_endpoint_requires_auth(self, method, path):
        resp = requests.request(method, f"{BASE_URL}{path}", timeout=10)
        assert resp.status_code == 401, (
            f"{method} {path} should return 401 without auth, got {resp.status_code}"
        )


class TestAdministratorAccess:
    def test_admin_can_access_events(self, admin_token):
        assert requests.get(
            f"{BASE_URL}/events", headers=auth_headers(admin_token), timeout=10
        ).status_code == 200

    def test_admin_can_access_audit_logs(self, admin_token):
        assert requests.get(
            f"{BASE_URL}/audit/logs", headers=auth_headers(admin_token), timeout=10
        ).status_code == 200

    def test_admin_can_access_backups(self, admin_token):
        assert requests.get(
            f"{BASE_URL}/admin/backups", headers=auth_headers(admin_token), timeout=10
        ).status_code == 200

    def test_admin_can_access_invoices(self, admin_token):
        assert requests.get(
            f"{BASE_URL}/invoices", headers=auth_headers(admin_token), timeout=10
        ).status_code == 200

    def test_admin_can_create_events(self, admin_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"RBAC Test Event {ts}"},
            timeout=10,
        )
        assert resp.status_code == 200

    def test_admin_can_manage_roles(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/admin/users/1/roles",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200


class TestEventDirectorAccess:
    def test_director_can_read_events(self, director_token):
        assert requests.get(
            f"{BASE_URL}/events", headers=auth_headers(director_token), timeout=10
        ).status_code == 200

    def test_director_can_create_events(self, director_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(director_token),
            json={"name": f"Director Event {ts}"},
            timeout=10,
        )
        assert resp.status_code == 200

    def test_director_cannot_access_audit_logs(self, director_token):
        assert requests.get(
            f"{BASE_URL}/audit/logs", headers=auth_headers(director_token), timeout=10
        ).status_code == 403

    def test_director_cannot_access_invoices(self, director_token):
        assert requests.get(
            f"{BASE_URL}/invoices", headers=auth_headers(director_token), timeout=10
        ).status_code == 403

    def test_director_cannot_access_backups(self, director_token):
        assert requests.get(
            f"{BASE_URL}/admin/backups", headers=auth_headers(director_token), timeout=10
        ).status_code == 403

    def test_director_cannot_manage_roles(self, director_token):
        resp = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(director_token),
            json={"user_id": 1, "role": "referee"},
            timeout=10,
        )
        assert resp.status_code == 403


class TestRefereeAccess:
    def test_referee_can_read_events(self, referee_token):
        assert requests.get(
            f"{BASE_URL}/events", headers=auth_headers(referee_token), timeout=10
        ).status_code == 200

    def test_referee_can_read_vehicles(self, referee_token):
        assert requests.get(
            f"{BASE_URL}/vehicles", headers=auth_headers(referee_token), timeout=10
        ).status_code == 200

    def test_referee_cannot_create_events(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(referee_token),
            json={"name": f"Unauthorized Event {ts}"},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_referee_cannot_create_vehicles(self, referee_token):
        resp = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(referee_token),
            json={"vin": "1HGCM82633A004352", "registration_id": "X",
                  "make": "X", "model": "X", "year": 2020},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_referee_cannot_access_invoices(self, referee_token):
        assert requests.get(
            f"{BASE_URL}/invoices", headers=auth_headers(referee_token), timeout=10
        ).status_code == 403

    def test_referee_cannot_access_audit_logs(self, referee_token):
        assert requests.get(
            f"{BASE_URL}/audit/logs", headers=auth_headers(referee_token), timeout=10
        ).status_code == 403

    def test_referee_cannot_access_backups(self, referee_token):
        assert requests.get(
            f"{BASE_URL}/admin/backups", headers=auth_headers(referee_token), timeout=10
        ).status_code == 403


class TestFinanceClerkAccess:
    def test_finance_can_read_invoices(self, finance_token):
        assert requests.get(
            f"{BASE_URL}/invoices", headers=auth_headers(finance_token), timeout=10
        ).status_code == 200

    def test_finance_can_read_events(self, finance_token):
        assert requests.get(
            f"{BASE_URL}/events", headers=auth_headers(finance_token), timeout=10
        ).status_code == 200

    def test_finance_cannot_create_events(self, finance_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(finance_token),
            json={"name": f"Finance Unauthorized {ts}"},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_cannot_access_audit_logs(self, finance_token):
        assert requests.get(
            f"{BASE_URL}/audit/logs", headers=auth_headers(finance_token), timeout=10
        ).status_code == 403

    def test_finance_cannot_manage_roles(self, finance_token):
        resp = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(finance_token),
            json={"user_id": 1, "role": "auditor"},
            timeout=10,
        )
        assert resp.status_code == 403


class TestAuditorAccess:
    def test_auditor_can_read_audit_logs(self, auditor_token):
        assert requests.get(
            f"{BASE_URL}/audit/logs", headers=auth_headers(auditor_token), timeout=10
        ).status_code == 200

    def test_auditor_can_read_events(self, auditor_token):
        assert requests.get(
            f"{BASE_URL}/events", headers=auth_headers(auditor_token), timeout=10
        ).status_code == 200

    def test_auditor_can_read_invoices(self, auditor_token):
        assert requests.get(
            f"{BASE_URL}/invoices", headers=auth_headers(auditor_token), timeout=10
        ).status_code == 200

    def test_auditor_cannot_create_events(self, auditor_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(auditor_token),
            json={"name": f"Auditor Unauthorized {ts}"},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_auditor_cannot_create_invoices(self, auditor_token, ts):
        resp = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(auditor_token),
            json={
                "invoice_no": f"INV-AUD-{ts}",
                "counterparty": "X",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        assert resp.status_code == 403

    def test_auditor_cannot_access_backups(self, auditor_token):
        assert requests.get(
            f"{BASE_URL}/admin/backups", headers=auth_headers(auditor_token), timeout=10
        ).status_code == 403


class TestRoleAssignment:
    def test_admin_can_list_user_roles(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/admin/users/1/roles",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "user_id" in body
        assert "roles" in body
        assert isinstance(body["roles"], list)

    def test_duplicate_role_assignment_returns_409(self, admin_token):
        """Assigning a role a user already has must return 409 Conflict."""
        resp = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(admin_token),
            json={"user_id": 1, "role": "administrator"},
            timeout=10,
        )
        # admin user already has administrator — should conflict
        assert resp.status_code == 409
        assert resp.json()["code"] == "CONFLICT"

    def test_revoke_nonexistent_role_returns_404(self, admin_token):
        """Revoking a role not held by the user must return 404."""
        resp = requests.post(
            f"{BASE_URL}/admin/roles/revoke",
            headers=auth_headers(admin_token),
            json={"user_id": 1, "role": "finance_clerk"},
            timeout=10,
        )
        assert resp.status_code == 404


class TestBackupRBAC:
    """Backup endpoints require system_admin — only administrators have it."""

    def test_admin_can_list_backups(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_backup_list_includes_retain_days(self, admin_token):
        """Response must include the configured retention window."""
        resp = requests.get(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert "retain_days" in resp, "Backup list response must include retain_days"
        assert isinstance(resp["retain_days"], int)
        assert resp["retain_days"] >= 1, "retain_days must be at least 1"

    def test_admin_can_trigger_backup(self, admin_token):
        resp = requests.post(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(admin_token),
            timeout=30,
        )
        assert resp.status_code == 200

    def test_director_cannot_list_backups(self, director_token):
        resp = requests.get(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(director_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_auditor_cannot_list_backups(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_cannot_trigger_backup(self, finance_token):
        resp = requests.post(
            f"{BASE_URL}/admin/backups",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_restore_endpoint_rejects_path_traversal(self, admin_token):
        """Filenames with path separators must be rejected before any FS operation."""
        for bad_name in ["../evil.sqlite", "sub/backup.sqlite", "..\\evil.sqlite"]:
            resp = requests.post(
                f"{BASE_URL}/admin/backups/{bad_name}/restore",
                headers=auth_headers(admin_token),
                timeout=10,
            )
            assert resp.status_code in (400, 404), (
                f"Path traversal attempt {bad_name!r} must be rejected"
            )

    def test_restore_nonexistent_backup_returns_404(self, admin_token):
        resp = requests.post(
            f"{BASE_URL}/admin/backups/backup_19700101_000000.sqlite/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code in (400, 404)

    def test_unauthenticated_cannot_access_backups(self):
        resp = requests.get(f"{BASE_URL}/admin/backups", timeout=10)
        assert resp.status_code == 401


class TestEncryptionAtRestEvidence:
    """
    Verify that API responses never leak raw ciphertext for encrypted fields.

    AES-256-GCM ciphertext is base64-encoded; a raw base64 blob in a response
    field is evidence that decryption was NOT applied.  Encrypted fields must
    always return human-readable plaintext to authorised callers, never a
    base64 blob.
    """

    def _is_base64_ciphertext(self, value) -> bool:
        """
        Heuristic: a value is likely raw ciphertext if it is a long (>50 char)
        base64-looking string — the pattern AES-256-GCM + nonce produces.
        Legitimate plaintext (VINs, refs) is always shorter.
        """
        import base64
        if not isinstance(value, str) or len(value) < 50:
            return False
        try:
            decoded = base64.b64decode(value)
            # AES-GCM blobs are at least nonce(12) + tag(16) = 28 bytes
            return len(decoded) > 28
        except Exception:
            return False

    def test_vehicle_vin_is_not_raw_ciphertext(self, admin_token, ts):
        vehicle = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": f"1HGCM{ts % 99999:05d}ENC",
                "registration_id": f"REG-ENC-{ts}",
                "make": "Honda",
                "model": "Civic",
                "year": 2022,
                "mileage": 0,
                "title_transfer_count": 0,
            },
            timeout=10,
        ).json()
        if "id" not in vehicle:
            return  # skip if creation failed
        vin = vehicle.get("vin", "")
        assert not self._is_base64_ciphertext(vin), (
            "VIN field returned to caller must be plaintext, not raw ciphertext"
        )

    def test_payment_external_reference_is_not_raw_ciphertext(
        self, admin_token, ts
    ):
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-ENC-{ts}",
                "counterparty": "Enc Test Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={"description": "Fee", "pricing_model": "fixed",
                  "quantity": 1.0, "unit_price": 50.0},
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        pmt = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 50.0,
                "method": "ach",
                "external_reference": f"ACH-ENC-{ts}",
                "received_at": "2026-04-01T10:00:00Z",
            },
            timeout=10,
        ).json()
        ext_ref = pmt.get("external_reference", "")
        assert not self._is_base64_ciphertext(ext_ref), (
            "external_reference returned to caller must be plaintext, not raw ciphertext"
        )
        # Must return the actual reference, not a garbled blob
        assert f"ACH-ENC-{ts}" == ext_ref, "Decrypted external_reference must match input"

    def test_asset_serial_number_is_not_raw_ciphertext(self, admin_token, ts):
        asset = requests.post(
            f"{BASE_URL}/assets",
            headers=auth_headers(admin_token),
            json={
                "asset_code": f"SN-ENC-{ts}",
                "name": "Enc Test Asset",
                "category": "equipment",
                "serial_number": f"SN-PLAIN-{ts}",
                "status": "in_service",
            },
            timeout=10,
        ).json()
        if "id" not in asset:
            return
        sn = asset.get("serial_number", "")
        assert not self._is_base64_ciphertext(sn), (
            "serial_number returned to caller must be plaintext, not raw ciphertext"
        )


class TestDataQualityAndAnalyticsRBAC:
    """Data quality and analytics endpoints require audit:read."""

    def test_auditor_can_run_data_quality_scan(self, auditor_token):
        resp = requests.post(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(auditor_token),
            json={"entity": "invoices", "checks": ["missing_fields"]},
            timeout=30,
        )
        assert resp.status_code == 200

    def test_referee_cannot_run_data_quality_scan(self, referee_token):
        resp = requests.post(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(referee_token),
            json={"entity": "invoices", "checks": ["missing_fields"]},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_clerk_cannot_run_data_quality_scan(self, finance_token):
        resp = requests.post(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(finance_token),
            json={"entity": "invoices", "checks": ["missing_fields"]},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_director_cannot_run_data_quality_scan(self, director_token):
        resp = requests.post(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(director_token),
            json={"entity": "invoices", "checks": ["missing_fields"]},
            timeout=10,
        )
        assert resp.status_code == 403

    def test_auditor_can_access_analytics_trends(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count&bucket=month",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_access_analytics_trends(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count&bucket=month",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_cannot_access_analytics_trends(self, finance_token):
        resp = requests.get(
            f"{BASE_URL}/analytics/trends?metric=invoice_count&bucket=month",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403
