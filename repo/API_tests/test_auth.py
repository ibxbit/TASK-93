"""
API tests for authentication endpoints.

Covers:
- POST /auth/login   (success, bad credentials, missing fields)
- POST /auth/logout  (success, missing token)
- GET  /health       (always public)
- Authorization header enforcement across protected endpoints
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


class TestHealthEndpoint:
    def test_health_returns_ok(self):
        resp = requests.get(f"{BASE_URL}/health", timeout=10)
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "ok"
        assert "version" in body

    def test_health_is_public(self):
        """Health check must work without any authentication."""
        resp = requests.get(f"{BASE_URL}/health", timeout=10)
        assert resp.status_code == 200


class TestLogin:
    def test_admin_login_success(self):
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "admin", "password": "Admin123!"},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "token" in body
        assert "expires_at" in body
        assert len(body["token"]) == 64  # two UUID4 simple concatenated

    def test_director_login_success(self):
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "director", "password": "Director123!"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert "token" in resp.json()

    def test_wrong_password_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "admin", "password": "wrong-password"},
            timeout=10,
        )
        assert resp.status_code == 401
        body = resp.json()
        assert body["code"] == "UNAUTHORIZED"

    def test_unknown_user_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "nonexistent_user_xyz", "password": "any"},
            timeout=10,
        )
        assert resp.status_code == 401
        body = resp.json()
        assert body["code"] == "UNAUTHORIZED"

    def test_error_message_does_not_distinguish_user_vs_password(self):
        """Anti-enumeration: same error for bad user vs bad password."""
        bad_user = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "nobody_xyz", "password": "pass"},
            timeout=10,
        ).json()
        bad_pass = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "admin", "password": "wrong"},
            timeout=10,
        ).json()
        assert bad_user["code"] == bad_pass["code"] == "UNAUTHORIZED"


class TestLogout:
    def test_logout_with_valid_token_succeeds(self, admin_token):
        # Login a fresh session just for logout
        resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "director", "password": "Director123!"},
            timeout=10,
        )
        temp_token = resp.json()["token"]

        logout_resp = requests.post(
            f"{BASE_URL}/auth/logout",
            headers=auth_headers(temp_token),
            timeout=10,
        )
        assert logout_resp.status_code == 200

    def test_logout_without_token_returns_401(self):
        resp = requests.post(f"{BASE_URL}/auth/logout", timeout=10)
        assert resp.status_code == 401

    def test_token_invalid_after_logout(self):
        """Using a token after logout must return 401."""
        # Create a fresh session
        login_resp = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "referee1", "password": "Referee123!"},
            timeout=10,
        )
        token = login_resp.json()["token"]

        # Logout
        requests.post(
            f"{BASE_URL}/auth/logout",
            headers=auth_headers(token),
            timeout=10,
        )

        # Try to use the same token
        resp = requests.get(
            f"{BASE_URL}/events",
            headers=auth_headers(token),
            timeout=10,
        )
        assert resp.status_code == 401


class TestAuthorizationEnforcement:
    def test_missing_auth_header_returns_401(self):
        resp = requests.get(f"{BASE_URL}/events", timeout=10)
        assert resp.status_code == 401
        assert resp.json()["code"] == "UNAUTHORIZED"

    def test_malformed_bearer_token_returns_401(self):
        resp = requests.get(
            f"{BASE_URL}/events",
            headers={"Authorization": "NotBearer abc123"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_empty_bearer_token_returns_401(self):
        resp = requests.get(
            f"{BASE_URL}/events",
            headers={"Authorization": "Bearer "},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_invalid_token_returns_401(self):
        resp = requests.get(
            f"{BASE_URL}/events",
            headers={"Authorization": "Bearer invalid_token_abc123"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_valid_token_allows_access(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
