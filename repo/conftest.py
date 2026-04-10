"""
Shared fixtures for API tests.

Provides:
- `base_url`          — API root URL
- `admin_token`       — session token for the Administrator user
- `director_token`    — session token for the EventDirector user
- `referee_token`     — session token for the Referee user
- `finance_token`     — session token for the FinanceClerk user
- `auditor_token`     — session token for the Auditor user
- `ts`                — unique integer timestamp prefix for this test run
                        (use in names to ensure idempotency across runs)
"""

import os
import time
import pytest
import requests

BASE_URL = os.environ.get("API_URL", "http://localhost:8000")

# Default seeded credentials (see src/auth/seeder.rs)
CREDENTIALS = {
    "admin":    ("admin",    "Admin123!"),
    "director": ("director", "Director123!"),
    "referee":  ("referee1", "Referee123!"),
    "finance":  ("finance1", "Finance123!"),
    "auditor":  ("auditor1", "Auditor123!"),
}


def _login(username: str, password: str) -> str:
    """Login and return the bearer token."""
    resp = requests.post(
        f"{BASE_URL}/auth/login",
        json={"username": username, "password": password},
        timeout=10,
    )
    assert resp.status_code == 200, (
        f"Login failed for {username}: {resp.status_code} {resp.text}"
    )
    token = resp.json().get("token")
    assert token, f"No token in login response for {username}"
    return token


@pytest.fixture(scope="session")
def base_url():
    return BASE_URL


@pytest.fixture(scope="session")
def ts():
    """Unique timestamp prefix for this test session — ensures idempotency."""
    return int(time.time())


@pytest.fixture(scope="session")
def admin_token():
    u, p = CREDENTIALS["admin"]
    return _login(u, p)


@pytest.fixture(scope="session")
def director_token():
    u, p = CREDENTIALS["director"]
    return _login(u, p)


@pytest.fixture(scope="session")
def referee_token():
    u, p = CREDENTIALS["referee"]
    return _login(u, p)


@pytest.fixture(scope="session")
def finance_token():
    u, p = CREDENTIALS["finance"]
    return _login(u, p)


@pytest.fixture(scope="session")
def auditor_token():
    u, p = CREDENTIALS["auditor"]
    return _login(u, p)


def auth_headers(token: str) -> dict:
    """Return Authorization header dict for a given token."""
    return {"Authorization": f"Bearer {token}"}
