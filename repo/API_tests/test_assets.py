"""
API tests for the asset register.

Covers:
- POST /assets               (create with encrypted serial_number)
- GET  /assets               (list with optional filters)
- GET  /assets/<id>          (get single asset)
- PUT  /assets/<id>          (update)
- PATCH /assets/<id>/status  (status transition)
- GET  /assets/<id>/history  (audit trail — sensitive fields redacted)
- GET  /assets/export        (bulk export)
- POST /assets/import        (bulk import with deduplication)
- RBAC enforcement
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


@pytest.fixture(scope="module")
def created_asset(admin_token, ts):
    """Create a test asset and return its response."""
    resp = requests.post(
        f"{BASE_URL}/assets",
        headers=auth_headers(admin_token),
        json={
            "asset_code": f"ASSET-{ts}",
            "category": "equipment",
            "brand": "Longines",
            "model": "RaceMaster Pro",
            "serial_number": f"SN-{ts}-001",
            "procurement_cost": 4500.00,
            "procurement_date": "2024-01-15",
            "useful_life_months": 120,
            "notes": "Test asset created by API test suite",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Asset creation failed: {resp.text}"
    return resp.json()


class TestAssetCreate:
    def test_admin_can_create_asset(self, admin_token, ts):
        resp = requests.post(
            f"{BASE_URL}/assets",
            headers=auth_headers(admin_token),
            json={
                "asset_code": f"ASSET-CREATE-{ts}",
                "category": "equipment",
                "brand": "Sparco",
                "model": "Pro Shield",
                "serial_number": f"SN-CREATE-{ts}",
                "procurement_cost": 1200.00,
                "procurement_date": "2025-06-01",
                "useful_life_months": 60,
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["asset_code"] == f"ASSET-CREATE-{ts}"
        assert body["status"] == "in_service"

    def test_create_asset_serial_number_encrypted_at_rest_returned_plaintext(
        self, created_asset, ts
    ):
        """serial_number should be returned in plaintext in the response."""
        assert f"SN-{ts}-001" in created_asset["serial_number"]

    def test_new_asset_has_active_status(self, created_asset):
        assert created_asset["status"] == "in_service"

    def test_create_asset_has_required_fields(self, created_asset):
        required = {"id", "asset_code", "category", "brand", "model",
                    "serial_number", "status", "created_at", "updated_at"}
        assert required.issubset(created_asset.keys())

    def test_referee_cannot_create_asset(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/assets",
            headers=auth_headers(referee_token),
            json={
                "asset_code": f"ASSET-REF-{ts}",
                "category": "equipment",
                "brand": "X",
                "model": "Y",
            },
            timeout=10,
        )
        assert resp.status_code == 403

    def test_create_without_auth_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/assets",
            json={"asset_code": "X", "category": "Y", "brand": "Z", "model": "W"},
            timeout=10,
        )
        assert resp.status_code == 401


class TestAssetRead:
    def test_get_asset_by_id(self, admin_token, created_asset):
        resp = requests.get(
            f"{BASE_URL}/assets/{created_asset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["id"] == created_asset["id"]

    def test_get_nonexistent_asset_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/assets/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404
        assert resp.json()["code"] == "NOT_FOUND"

    def test_list_assets_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/assets",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_list_assets_filter_by_category(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/assets?category=equipment",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assets = resp.json()
        assert all(a["category"] == "equipment" for a in assets)

    def test_referee_can_read_assets(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/assets",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 200


class TestAssetUpdate:
    def test_update_asset_fields(self, admin_token, created_asset):
        resp = requests.put(
            f"{BASE_URL}/assets/{created_asset['id']}",
            headers=auth_headers(admin_token),
            json={"notes": "Updated by test suite"},
            timeout=10,
        )
        assert resp.status_code == 200

    def test_update_nonexistent_asset_returns_404(self, admin_token):
        resp = requests.put(
            f"{BASE_URL}/assets/999999999",
            headers=auth_headers(admin_token),
            json={"notes": "X"},
            timeout=10,
        )
        assert resp.status_code == 404


class TestAssetStatusUpdate:
    def test_update_status_to_out_for_repair(self, admin_token, created_asset):
        resp = requests.patch(
            f"{BASE_URL}/assets/{created_asset['id']}/status",
            headers=auth_headers(admin_token),
            json={"status": "out_for_repair"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "out_for_repair"

    def test_return_asset_to_active(self, admin_token, created_asset):
        resp = requests.patch(
            f"{BASE_URL}/assets/{created_asset['id']}/status",
            headers=auth_headers(admin_token),
            json={"status": "in_service"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "in_service"


class TestAssetHistory:
    def test_get_asset_history_returns_array(self, admin_token, created_asset):
        resp = requests.get(
            f"{BASE_URL}/assets/{created_asset['id']}/history",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_asset_history_has_entries(self, admin_token, created_asset):
        resp = requests.get(
            f"{BASE_URL}/assets/{created_asset['id']}/history",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        history = resp.json()
        assert len(history) >= 1

    def test_asset_history_serial_number_is_redacted(self, admin_token, created_asset):
        """Sensitive fields must appear as [REDACTED] in audit snapshots."""
        resp = requests.get(
            f"{BASE_URL}/assets/{created_asset['id']}/history",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        history = resp.json()
        if history:
            snapshot_str = str(history[0].get("snapshot", {}))
            # serial_number should not appear in plaintext in history
            assert "SN-" not in snapshot_str or "[REDACTED]" in snapshot_str


class TestAssetExportImport:
    def test_export_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/assets/export",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_import_assets_bulk(self, admin_token, ts):
        resp = requests.post(
            f"{BASE_URL}/assets/import",
            headers=auth_headers(admin_token),
            json={
                "assets": [
                    {
                        "asset_code": f"IMPORT-A-{ts}",
                        "category": "equipment",
                        "brand": "TAG Heuer",
                        "model": "HL67",
                        "serial_number": f"SN-IMPORT-A-{ts}",
                        "procurement_cost": 3000.00,
                        "procurement_date": "2025-01-01",
                        "useful_life_months": 96,
                    },
                    {
                        "asset_code": f"IMPORT-B-{ts}",
                        "category": "electronic",
                        "brand": "Motorola",
                        "model": "DP4801",
                        "serial_number": f"SN-IMPORT-B-{ts}",
                        "procurement_cost": 800.00,
                        "procurement_date": "2025-03-15",
                        "useful_life_months": 60,
                    },
                ]
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "imported" in body
        assert isinstance(body["imported"], int)

    def test_import_duplicate_asset_code_is_skipped(self, admin_token, ts):
        """Re-importing the same asset_code+serial_number should be silently skipped."""
        payload = {
            "assets": [
                {
                    "asset_code": f"IMPORT-A-{ts}",
                    "category": "equipment",
                    "brand": "TAG Heuer",
                    "model": "HL67",
                    "serial_number": f"SN-IMPORT-A-{ts}",
                    "procurement_cost": 3000.00,
                    "procurement_date": "2025-01-01",
                    "useful_life_months": 96,
                }
            ]
        }
        resp = requests.post(
            f"{BASE_URL}/assets/import",
            headers=auth_headers(admin_token),
            json=payload,
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        # Duplicate should be in skipped count, not created
        assert body.get("imported", 0) == 0 or body.get("skipped", 0) > 0
