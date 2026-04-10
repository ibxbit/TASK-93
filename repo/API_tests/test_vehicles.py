"""
API tests for vehicle lifecycle management.

Covers:
- POST /vehicles           (create)
- GET  /vehicles           (list)
- GET  /vehicles/<id>      (get)
- PUT  /vehicles/<id>      (update)
- POST /vehicles/<id>/status (lifecycle transitions)
- GET  /vehicles/<id>/history
- RBAC enforcement
- Error scenarios
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


def make_vin(ts: int) -> str:
    """
    Generate a valid 17-char VIN from a timestamp.
    VINs must be uppercase alphanumeric, no I/O/Q.
    Pad/truncate to exactly 17 chars.
    """
    raw = f"1HGTEST{ts}"[-17:].upper()
    # Replace any I, O, Q that might appear
    raw = raw.replace("I", "1").replace("O", "0").replace("Q", "9")
    # Pad with zeros if needed (shouldn't be for reasonable ts values)
    return raw.ljust(17, "0")[:17]


@pytest.fixture(scope="module")
def created_vehicle(admin_token, ts):
    """Create a vehicle and return its response body."""
    vin = make_vin(ts)
    resp = requests.post(
        f"{BASE_URL}/vehicles",
        headers=auth_headers(admin_token),
        json={
            "vin": vin,
            "registration_id": f"REG-{ts}",
            "make": "Honda",
            "model": "Accord",
            "year": 2023,
            "color": "Blue",
            "mileage": 0,
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Vehicle creation failed: {resp.text}"
    return resp.json()


class TestVehicleCreate:
    def test_admin_can_create_vehicle(self, admin_token, ts):
        vin = f"2HGCM{ts % 100000:05d}A000001"[:17].replace("I", "1").replace("O", "0").replace("Q", "9")
        resp = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": vin,
                "registration_id": f"REG-CREATE-{ts}",
                "make": "Toyota",
                "model": "Camry",
                "year": 2024,
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["make"] == "Toyota"
        assert body["status"] == "draft"
        assert body["id"] > 0

    def test_create_vehicle_returns_correct_fields(self, created_vehicle):
        v = created_vehicle
        required = {"id", "vin", "registration_id", "make", "model", "year",
                    "status", "mileage", "title_transfer_count", "created_by",
                    "created_at", "updated_at"}
        assert required.issubset(v.keys())

    def test_new_vehicle_starts_in_draft_status(self, created_vehicle):
        assert created_vehicle["status"] == "draft"

    def test_create_without_auth_returns_401(self, ts):
        resp = requests.post(
            f"{BASE_URL}/vehicles",
            json={"vin": "1HGCM82633A004352", "registration_id": "X",
                  "make": "X", "model": "X", "year": 2020},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_referee_cannot_create_vehicle(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(referee_token),
            json={"vin": "1HGCM82633A004353", "registration_id": "X",
                  "make": "X", "model": "X", "year": 2020},
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_auditor_cannot_create_vehicle(self, auditor_token):
        resp = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(auditor_token),
            json={"vin": "1HGCM82633A004354", "registration_id": "X",
                  "make": "X", "model": "X", "year": 2020},
            timeout=10,
        )
        assert resp.status_code == 403


class TestVehicleRead:
    def test_get_vehicle_by_id(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        resp = requests.get(
            f"{BASE_URL}/vehicles/{vid}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["id"] == vid

    def test_get_nonexistent_vehicle_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/vehicles/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404
        assert resp.json()["code"] == "NOT_FOUND"

    def test_list_vehicles_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_list_vehicles_filter_by_status(self, admin_token, created_vehicle):
        resp = requests.get(
            f"{BASE_URL}/vehicles?status=draft",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        vehicles = resp.json()
        assert isinstance(vehicles, list)
        assert all(v["status"] == "draft" for v in vehicles)

    def test_referee_can_list_vehicles(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_list_vehicles_without_auth_returns_401(self):
        resp = requests.get(f"{BASE_URL}/vehicles", timeout=10)
        assert resp.status_code == 401


class TestVehicleUpdate:
    def test_update_vehicle_fields(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        resp = requests.put(
            f"{BASE_URL}/vehicles/{vid}",
            headers=auth_headers(admin_token),
            json={"color": "Red", "notes": "Updated in test"},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["color"] == "Red"

    def test_update_nonexistent_vehicle_returns_404(self, admin_token):
        resp = requests.put(
            f"{BASE_URL}/vehicles/999999999",
            headers=auth_headers(admin_token),
            json={"color": "Green"},
            timeout=10,
        )
        assert resp.status_code == 404


class TestVehicleStatusTransition:
    def test_transition_draft_to_published(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        resp = requests.post(
            f"{BASE_URL}/vehicles/{vid}/status",
            headers=auth_headers(admin_token),
            json={"status": "published"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "published"

    def test_transition_published_to_sold(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        # First ensure it's published
        current = requests.get(
            f"{BASE_URL}/vehicles/{vid}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()

        if current["status"] == "published":
            resp = requests.post(
                f"{BASE_URL}/vehicles/{vid}/status",
                headers=auth_headers(admin_token),
                json={"status": "sold", "reason": "Test sale"},
                timeout=10,
            )
            assert resp.status_code == 200
            assert resp.json()["status"] == "sold"


class TestVehicleHistory:
    def test_get_vehicle_history_returns_array(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        resp = requests.get(
            f"{BASE_URL}/vehicles/{vid}/history",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_vehicle_history_has_initial_entry(self, admin_token, created_vehicle):
        vid = created_vehicle["id"]
        resp = requests.get(
            f"{BASE_URL}/vehicles/{vid}/history",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        history = resp.json()
        assert len(history) >= 1
        assert history[0]["vehicle_id"] == vid
