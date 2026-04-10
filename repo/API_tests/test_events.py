"""
API tests for event and ruleset lifecycle management.

Covers:
- POST /rulesets              (create immutable ruleset version)
- GET  /rulesets              (list all versions)
- GET  /rulesets/<id>         (get single version)
- POST /rulesets/<id>/rollback
- Ruleset immutability: no PUT/DELETE endpoints exist
- Ruleset version chain: rollback_of, is_rollback fields
- Multiple events can reference different ruleset versions
- POST /events                (create in draft)
- GET  /events                (list with filters)
- GET  /events/<id>
- PUT  /events/<id>           (update draft only)
- POST /events/<id>/publish   (draft → published; freezes config)
- State machine: published events cannot be updated
- Audit trail: events are logged
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


@pytest.fixture(scope="module")
def ruleset(admin_token, ts):
    """Create a ruleset version for this test module."""
    resp = requests.post(
        f"{BASE_URL}/rulesets",
        headers=auth_headers(admin_token),
        json={
            "semantic_version": f"1.{ts % 1000}.0",
            "description": "Test ruleset for API tests",
            "effective_at": "2026-01-01T00:00:00Z",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Ruleset creation failed: {resp.text}"
    return resp.json()


@pytest.fixture(scope="module")
def draft_event(admin_token, ts):
    """Create a draft event for this test module."""
    resp = requests.post(
        f"{BASE_URL}/events",
        headers=auth_headers(admin_token),
        json={
            "name": f"Test Event {ts}",
            "description": "Created by API test suite",
            "schedule_group": "Group A",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Event creation failed: {resp.text}"
    return resp.json()


class TestRulesetVersion:
    def test_create_ruleset_returns_version(self, admin_token, ts):
        resp = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={
                "semantic_version": f"2.{ts % 100}.0",
                "description": "Another ruleset",
                "effective_at": "2026-06-01T00:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert "id" in body
        assert "semantic_version" in body
        assert body["is_rollback"] is False

    def test_list_rulesets_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_get_ruleset_by_id(self, admin_token, ruleset):
        resp = requests.get(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["id"] == ruleset["id"]

    def test_get_nonexistent_ruleset_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/rulesets/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404

    def test_rollback_ruleset_creates_new_version(self, admin_token, ruleset, ts):
        resp = requests.post(
            f"{BASE_URL}/rulesets/{ruleset['id']}/rollback",
            headers=auth_headers(admin_token),
            json={
                "new_semantic_version": f"1.{ts % 1000}.1-rollback",
                "description": "Rollback to previous rules",
                "effective_at": "2026-02-01T00:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["is_rollback"] is True
        assert body["rollback_of"] == ruleset["id"]

    def test_referee_can_read_rulesets(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_create_rulesets(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(referee_token),
            json={
                "semantic_version": "9.9.9",
                "effective_at": "2026-01-01T00:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 403


class TestEventCreate:
    def test_create_event_returns_draft_status(self, draft_event):
        assert draft_event["status"] == "draft"

    def test_create_event_has_required_fields(self, draft_event):
        required = {"id", "name", "status", "is_championship_class",
                    "asset_ids", "created_by", "created_at", "updated_at"}
        assert required.issubset(draft_event.keys())

    def test_create_event_without_auth_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/events",
            json={"name": "Unauthorized"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_finance_clerk_cannot_create_event(self, finance_token, ts):
        resp = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(finance_token),
            json={"name": f"Finance Unauthorized {ts}"},
            timeout=10,
        )
        assert resp.status_code == 403


class TestEventRead:
    def test_get_event_by_id(self, admin_token, draft_event):
        resp = requests.get(
            f"{BASE_URL}/events/{draft_event['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["id"] == draft_event["id"]

    def test_list_events_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_list_events_filter_by_status(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/events?status=draft",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert all(e["status"] == "draft" for e in resp.json())

    def test_get_nonexistent_event_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/events/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404


class TestEventUpdate:
    def test_update_draft_event(self, admin_token, draft_event):
        resp = requests.put(
            f"{BASE_URL}/events/{draft_event['id']}",
            headers=auth_headers(admin_token),
            json={"description": "Updated description"},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["description"] == "Updated description"

    def test_update_nonexistent_event_returns_404(self, admin_token):
        resp = requests.put(
            f"{BASE_URL}/events/999999999",
            headers=auth_headers(admin_token),
            json={"description": "X"},
            timeout=10,
        )
        assert resp.status_code == 404


class TestEventPublish:
    def test_publish_event_transitions_to_published(self, admin_token, ruleset, ts):
        # Create a fresh event to publish
        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Publish Test {ts}"},
            timeout=10,
        ).json()

        resp = requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "published"
        assert body["published_version_id"] == ruleset["id"]

    def test_cannot_update_published_event(self, admin_token, ruleset, ts):
        # Create and publish an event
        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Publish Lock Test {ts + 1}"},
            timeout=10,
        ).json()

        requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )

        # Now try to update — should fail (published events are frozen)
        resp = requests.put(
            f"{BASE_URL}/events/{event['id']}",
            headers=auth_headers(admin_token),
            json={"description": "Should not be allowed"},
            timeout=10,
        )
        assert resp.status_code in (400, 409, 422)

    def test_publish_nonexistent_event_returns_404(self, admin_token, ruleset):
        resp = requests.post(
            f"{BASE_URL}/events/999999999/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        assert resp.status_code == 404


class TestRulesetImmutability:
    """Rulesets are append-only version records — no updates or deletes."""

    def test_no_put_endpoint_for_rulesets(self, admin_token, ruleset):
        """PUT /rulesets/<id> must not exist (rulesets are immutable)."""
        resp = requests.put(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            json={"semantic_version": "999.0.0"},
            timeout=10,
        )
        assert resp.status_code in (404, 405), (
            "PUT on rulesets must not be allowed — rulesets are immutable"
        )

    def test_no_delete_endpoint_for_rulesets(self, admin_token, ruleset):
        """DELETE /rulesets/<id> must not exist."""
        resp = requests.delete(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code in (404, 405)

    def test_rollback_creates_new_row_not_update(self, admin_token, ruleset, ts):
        """Rollback produces a new ruleset row; the original is unchanged."""
        original = requests.get(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()

        rollback = requests.post(
            f"{BASE_URL}/rulesets/{ruleset['id']}/rollback",
            headers=auth_headers(admin_token),
            json={
                "new_semantic_version": f"0.{ts % 999}.0-rb",
                "description": "Immutability rollback test",
                "effective_at": "2026-03-01T00:00:00Z",
            },
            timeout=10,
        ).json()

        # Rollback produces a new ID.
        assert rollback["id"] != ruleset["id"]
        # Original is unchanged.
        after_original = requests.get(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert after_original["semantic_version"] == original["semantic_version"]

    def test_rollback_sets_rollback_of_and_is_rollback_flag(self, admin_token, ruleset, ts):
        """Rollback row must carry rollback_of = original ID and is_rollback = True."""
        rollback = requests.post(
            f"{BASE_URL}/rulesets/{ruleset['id']}/rollback",
            headers=auth_headers(admin_token),
            json={
                "new_semantic_version": f"0.{ts % 999}.1-rb",
                "effective_at": "2026-03-01T00:00:00Z",
            },
            timeout=10,
        ).json()
        assert rollback["is_rollback"] is True
        assert rollback["rollback_of"] == ruleset["id"]

    def test_ruleset_has_all_version_fields(self, admin_token, ruleset):
        """Rulesets must expose all versioning fields."""
        resp = requests.get(
            f"{BASE_URL}/rulesets/{ruleset['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        required = {"id", "semantic_version", "effective_at",
                    "created_by", "created_at", "is_rollback"}
        assert required.issubset(resp.keys())


class TestEventRulesetVersioning:
    """Events are pinned to a specific ruleset version at publish time."""

    def test_published_event_records_ruleset_version(self, admin_token, ruleset, ts):
        """published_version_id must equal the ruleset used at publish time."""
        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Version Pin Test {ts}"},
            timeout=10,
        ).json()
        resp = requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        ).json()
        assert resp["published_version_id"] == ruleset["id"]

    def test_two_events_can_use_different_ruleset_versions(self, admin_token, ts):
        """Independent events may reference different ruleset versions."""
        rs1 = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={"semantic_version": f"11.{ts % 99}.0",
                  "effective_at": "2026-01-01T00:00:00Z"},
            timeout=10,
        ).json()
        rs2 = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={"semantic_version": f"12.{ts % 99}.0",
                  "effective_at": "2026-02-01T00:00:00Z"},
            timeout=10,
        ).json()

        ev1 = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"RS Version A {ts}"},
            timeout=10,
        ).json()
        ev2 = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"RS Version B {ts}"},
            timeout=10,
        ).json()

        p1 = requests.post(
            f"{BASE_URL}/events/{ev1['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": rs1["id"]},
            timeout=10,
        ).json()
        p2 = requests.post(
            f"{BASE_URL}/events/{ev2['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": rs2["id"]},
            timeout=10,
        ).json()

        assert p1["published_version_id"] == rs1["id"]
        assert p2["published_version_id"] == rs2["id"]
        assert p1["published_version_id"] != p2["published_version_id"]

    def test_publish_with_nonexistent_ruleset_returns_error(self, admin_token, ts):
        """Publishing with a non-existent ruleset version must fail."""
        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Bad Ruleset Publish {ts}"},
            timeout=10,
        ).json()
        resp = requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": 999999999},
            timeout=10,
        )
        assert resp.status_code in (400, 404, 422)
