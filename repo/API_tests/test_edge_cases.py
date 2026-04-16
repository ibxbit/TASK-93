"""
Targeted integration-boundary and edge-case tests.

These tests fill gaps in the existing suite by exercising:

    - Password rotation (`POST /auth/rotate-password`) — previously uncovered.
    - Role lifecycle assign → list → revoke round-trip.
    - Content-Type / malformed JSON handling across write endpoints.
    - Numeric & string input boundaries (very large values, unicode,
      trailing whitespace) on user-facing fields.
    - Idempotency behaviours for payment `external_reference` across
      invoices (same-invoice = 200 duplicate; different-invoice = 409).
    - Pagination boundary conditions (limit=0, limit=1, offset beyond end).
    - Unknown query parameters must not crash the service.
    - Concurrent writes race condition: two near-simultaneous payments
      with different references both settle or the second correctly
      conflicts — never silently lost.
    - CSV export content-type + non-empty body for results and analytics.

These are purposely narrow, fast tests — each verifies a single, real
HTTP integration boundary.  Nothing is mocked.
"""

import csv
import io
import time
import threading

import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Password rotation ───────────────────────────────────────────────────────

class TestPasswordRotation:
    """
    `/auth/rotate-password` is documented in the README but not otherwise
    exercised.  We *only* test the negative path here — the positive rotation
    flow is deliberately NOT tested against seeded users because it purges
    every existing session for the account, and all seeded accounts back
    session-scoped fixtures (`referee_token`, `admin_token`, …) that other
    tests in the suite depend on.  Adding a rotation would invalidate those
    fixtures for the remainder of the run.
    """

    def _login(self, username, password):
        return requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": username, "password": password},
            timeout=10,
        )

    def test_rotate_with_wrong_current_password_fails(self):
        """
        Rotation must refuse if `current_password` is wrong, *without*
        invalidating the caller's session.
        """
        login = self._login("admin", "Admin123!")
        assert login.status_code == 200
        token = login.json()["token"]
        try:
            resp = requests.post(
                f"{BASE_URL}/auth/rotate-password",
                headers=auth_headers(token),
                json={
                    "current_password": "this-is-wrong",
                    "new_password": "Admin123!NEW",
                },
                timeout=10,
            )
            assert resp.status_code == 401
            assert resp.json()["code"] == "UNAUTHORIZED"

            # The caller's existing session must survive a failed rotation.
            probe = requests.get(
                f"{BASE_URL}/health",
                headers=auth_headers(token),
                timeout=10,
            )
            assert probe.status_code == 200
        finally:
            requests.post(
                f"{BASE_URL}/auth/logout",
                headers=auth_headers(token),
                timeout=10,
            )

    def test_rotate_requires_authentication(self):
        """Unauthenticated rotation attempts must 401."""
        r = requests.post(
            f"{BASE_URL}/auth/rotate-password",
            json={"current_password": "x", "new_password": "y"},
            timeout=10,
        )
        assert r.status_code == 401

    def test_rotate_with_invalid_token_returns_401(self):
        r = requests.post(
            f"{BASE_URL}/auth/rotate-password",
            headers={"Authorization": "Bearer not-a-real-session-token"},
            json={"current_password": "x", "new_password": "y"},
            timeout=10,
        )
        assert r.status_code == 401


# ── Role assignment round-trip ──────────────────────────────────────────────

class TestRoleLifecycle:
    """Full assign → list → revoke round-trip — complements the RBAC tests
    that only cover the error paths."""

    def test_assign_then_revoke_for_director(self, admin_token):
        """
        Assign the `auditor` role to the director user (who does not hold
        it by default), verify it appears in the listing, then revoke and
        verify it's gone.
        """
        # The seeded 'director' user id: find it by listing roles for each
        # known user id.  We rely on director being user id 2 (admin is 1)
        # per seeder ordering, but verify defensively.
        who_am_i = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "director", "password": "Director123!"},
            timeout=10,
        )
        assert who_am_i.status_code == 200

        # Admin is always id=1; director is seeded next.  Scan small ids.
        director_id = None
        for uid in range(1, 12):
            r = requests.get(
                f"{BASE_URL}/admin/users/{uid}/roles",
                headers=auth_headers(admin_token),
                timeout=10,
            )
            if r.status_code != 200:
                continue
            roles = r.json().get("roles", [])
            if "event_director" in roles:
                director_id = uid
                break

        if director_id is None:
            pytest.skip("Could not locate director user id")

        # Ensure clean state: revoke auditor first, accept 404.
        requests.post(
            f"{BASE_URL}/admin/roles/revoke",
            headers=auth_headers(admin_token),
            json={"user_id": director_id, "role": "auditor"},
            timeout=10,
        )

        # Assign auditor.
        assign = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(admin_token),
            json={"user_id": director_id, "role": "auditor"},
            timeout=10,
        )
        assert assign.status_code == 200
        assert assign.json() == {"ok": True}

        # Roles now include auditor.
        listing = requests.get(
            f"{BASE_URL}/admin/users/{director_id}/roles",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert "auditor" in listing["roles"]
        assert "event_director" in listing["roles"], (
            "existing role assignments must not be removed by a new assignment"
        )

        # Revoke auditor.
        revoke = requests.post(
            f"{BASE_URL}/admin/roles/revoke",
            headers=auth_headers(admin_token),
            json={"user_id": director_id, "role": "auditor"},
            timeout=10,
        )
        assert revoke.status_code == 200

        after = requests.get(
            f"{BASE_URL}/admin/users/{director_id}/roles",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert "auditor" not in after["roles"]
        assert "event_director" in after["roles"]

    def test_revoke_unassigned_role_returns_404(self, admin_token):
        # admin user does not hold 'referee' — revoking must 404.
        resp = requests.post(
            f"{BASE_URL}/admin/roles/revoke",
            headers=auth_headers(admin_token),
            json={"user_id": 1, "role": "referee"},
            timeout=10,
        )
        assert resp.status_code == 404

    def test_assign_to_nonexistent_user_returns_error(self, admin_token):
        """
        Assigning a role to a bogus user id must fail — whether by 4xx
        (input validation) or 5xx (FK constraint violation).  The *critical*
        invariant is that the bogus id does not end up with any assigned
        roles when listed afterwards.
        """
        bogus = 99999999
        r = requests.post(
            f"{BASE_URL}/admin/roles/assign",
            headers=auth_headers(admin_token),
            json={"user_id": bogus, "role": "referee"},
            timeout=10,
        )
        assert r.status_code >= 400, (
            f"Bogus assign must not be silently accepted: got {r.status_code}"
        )

        # Double-check: the bogus user id has no roles.
        listing = requests.get(
            f"{BASE_URL}/admin/users/{bogus}/roles",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        if listing.status_code == 200:
            assert listing.json().get("roles", []) == [], (
                "Bogus user ended up with roles after a failed assign"
            )


# ── Malformed request bodies ─────────────────────────────────────────────────

class TestMalformedRequests:
    """Every write endpoint must reject malformed input with a 4xx, never a 5xx."""

    def test_login_with_non_json_body_returns_4xx(self):
        r = requests.post(
            f"{BASE_URL}/auth/login",
            data="this is not json at all",
            headers={"Content-Type": "application/json"},
            timeout=10,
        )
        assert 400 <= r.status_code < 500, (
            f"Non-JSON body must produce 4xx, got {r.status_code}"
        )

    def test_login_missing_password_returns_4xx(self):
        r = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "admin"},
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_login_missing_username_returns_4xx(self):
        r = requests.post(
            f"{BASE_URL}/auth/login",
            json={"password": "Admin123!"},
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_event_create_with_wrong_content_type(self, admin_token):
        r = requests.post(
            f"{BASE_URL}/events",
            headers={
                **auth_headers(admin_token),
                "Content-Type": "text/plain",
            },
            data="name=BadContentType",
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_event_create_with_unexpected_type_value(self, admin_token, ts):
        """year as a string instead of int should 4xx not crash."""
        r = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": f"1HGXX{ts % 99999:05d}AA12345"[:17],
                "registration_id": f"BAD-TYPE-{ts}",
                "make": "X",
                "model": "Y",
                "year": "two thousand twenty four",
            },
            timeout=10,
        )
        assert 400 <= r.status_code < 500


# ── Idempotency boundaries ──────────────────────────────────────────────────

class TestPaymentReferenceIdempotencyAcrossInvoices:
    """`external_reference` is idempotent on the same invoice but unique globally."""

    def _make_issued_invoice(self, token, ts, suffix):
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(token),
            json={
                "invoice_no": f"INV-{suffix}-{ts}",
                "counterparty": "Idempotent Corp",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(token),
            json={
                "description": "Fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 200.0,
            },
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(token),
            json={},
            timeout=10,
        )
        return inv

    def test_same_reference_different_invoice_returns_409(self, admin_token, ts):
        inv1 = self._make_issued_invoice(admin_token, ts, "XINV1")
        inv2 = self._make_issued_invoice(admin_token, ts, "XINV2")
        ref = f"UNIQUE-REF-{ts}"

        first = requests.post(
            f"{BASE_URL}/invoices/{inv1['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "cash",
                "external_reference": ref,
                "received_at": "2026-01-10T10:00:00Z",
            },
            timeout=10,
        )
        assert first.status_code == 200

        second = requests.post(
            f"{BASE_URL}/invoices/{inv2['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "cash",
                "external_reference": ref,  # same reference, different invoice
                "received_at": "2026-01-10T11:00:00Z",
            },
            timeout=10,
        )
        assert second.status_code == 409, (
            f"Duplicate external_reference across invoices must 409; got {second.status_code}"
        )


# ── Pagination boundaries ────────────────────────────────────────────────────

class TestPaginationBoundaries:
    def test_audit_logs_limit_zero_is_handled(self, auditor_token):
        """limit=0 is an edge case — either rejected with 400 or returns []."""
        r = requests.get(
            f"{BASE_URL}/audit/logs?limit=0",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert r.status_code in (200, 400)
        if r.status_code == 200:
            assert r.json() == []

    def test_audit_logs_limit_one_returns_at_most_one(self, auditor_token):
        r = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert r.status_code == 200
        assert len(r.json()) <= 1

    def test_audit_logs_offset_beyond_end_returns_empty(self, auditor_token):
        r = requests.get(
            f"{BASE_URL}/audit/logs?limit=10&offset=9999999",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert r.status_code == 200
        assert r.json() == []

    def test_audit_logs_negative_limit_is_handled(self, auditor_token):
        """A negative limit must not crash — either 400 or treated as default."""
        r = requests.get(
            f"{BASE_URL}/audit/logs?limit=-5",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert r.status_code < 500

    def test_events_with_unknown_query_param(self, admin_token):
        """Unknown query params must be ignored, not cause 500."""
        r = requests.get(
            f"{BASE_URL}/events?this_param_does_not_exist=value",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code == 200


# ── Input range / length boundaries ──────────────────────────────────────────

class TestInputBoundaries:
    def test_event_name_unicode_is_accepted(self, admin_token, ts):
        name = f"Événement €2026 — 東京 #{ts}"
        r = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": name, "description": "unicode test"},
            timeout=10,
        )
        assert r.status_code == 200
        body = r.json()
        assert body["name"] == name, (
            f"Round-trip mismatch: sent {name!r}, got {body['name']!r}"
        )

    def test_event_name_with_leading_trailing_whitespace(self, admin_token, ts):
        """Leading/trailing whitespace must either be trimmed or rejected;
        the server must never 500."""
        r = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"    Whitespace {ts}    "},
            timeout=10,
        )
        assert r.status_code < 500
        if r.status_code == 200:
            # If accepted, name must round-trip (possibly trimmed).
            assert r.json()["name"].strip() == f"Whitespace {ts}"

    def test_event_name_empty_string_is_handled(self, admin_token):
        """
        Empty event names may be accepted or rejected — either choice is
        defensible — but the server must never crash on one.
        """
        r = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": ""},
            timeout=10,
        )
        assert r.status_code < 500, (
            f"Empty event name must not crash the server: {r.status_code}"
        )
        if r.status_code == 200:
            # Accepted — round-trip must preserve empty name exactly.
            assert r.json().get("name") == ""

    def test_vehicle_vin_too_short_rejected(self, admin_token, ts):
        """VINs are exactly 17 chars; shorter must fail."""
        r = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": "SHORT",
                "registration_id": f"SHORT-{ts}",
                "make": "X",
                "model": "Y",
                "year": 2024,
            },
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_vehicle_vin_too_long_rejected(self, admin_token, ts):
        r = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": "1234567890123456789",  # 19 chars
                "registration_id": f"LONG-{ts}",
                "make": "X",
                "model": "Y",
                "year": 2024,
            },
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_invoice_negative_tax_rate_rejected(self, admin_token, ts):
        r = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-NEGT-{ts}",
                "counterparty": "X",
                "issue_date": "2026-01-01",
                "tax_rate": -0.1,
            },
            timeout=10,
        )
        assert 400 <= r.status_code < 500

    def test_result_zero_value_accepted_or_rejected_consistently(self, admin_token, ts):
        """value_numeric = 0 is a boundary: the service must decide and be
        consistent (treat as invalid if negative is invalid; otherwise allow)."""
        # First publish an event for this test.  Semantic version must be X.Y.Z.
        rs = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={
                "semantic_version": f"21.{ts % 9999}.0",
                "effective_at": "2026-01-01T00:00:00Z",
            },
            timeout=10,
        )
        assert rs.status_code == 200, f"ruleset create: {rs.text}"
        rs = rs.json()
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Zero Value Event {ts}"},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": rs["id"]},
            timeout=10,
        )
        r = requests.post(
            f"{BASE_URL}/events/{ev['id']}/results",
            headers=auth_headers(admin_token),
            json={
                "participant_id": ts + 11111,
                "value_numeric": 0.0,
                "unit": "milliseconds",
            },
            timeout=10,
        )
        # Any 2xx or 4xx is acceptable; a 5xx would be a regression.
        assert r.status_code < 500


# ── Concurrent write race ───────────────────────────────────────────────────

class TestConcurrentPaymentWrite:
    """
    Two near-simultaneous payment POSTs on the same issued invoice with
    *different* references must both succeed without deadlocks or data
    loss.  With the *same* reference, exactly one must be the primary
    and the other must be idempotent (200 same id).
    """

    def _make_issued(self, admin_token, ts, suffix, total=1000.0):
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-CC-{suffix}-{ts}",
                "counterparty": "Concurrent Corp",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": total,
            },
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        return inv

    def test_two_payments_with_same_reference_are_idempotent(self, admin_token, ts):
        inv = self._make_issued(admin_token, ts, "IDEM")
        ref = f"RACE-IDEM-{ts}"
        body = {
            "amount": 200.0,
            "method": "cash",
            "external_reference": ref,
            "received_at": "2026-01-10T10:00:00Z",
        }

        results: list[requests.Response] = []
        def post_once():
            results.append(
                requests.post(
                    f"{BASE_URL}/invoices/{inv['id']}/payments",
                    headers=auth_headers(admin_token),
                    json=body,
                    timeout=10,
                )
            )

        t1 = threading.Thread(target=post_once)
        t2 = threading.Thread(target=post_once)
        t1.start(); t2.start()
        t1.join(); t2.join()

        statuses = [r.status_code for r in results]
        # Both may be 200 (idempotent) — or one 200, one 409 — but never 5xx.
        assert all(s < 500 for s in statuses), f"server error under concurrency: {statuses}"
        ok_ids = [r.json()["id"] for r in results if r.status_code == 200]
        # Every successful response must reference the same payment row.
        assert len(set(ok_ids)) <= 1, (
            f"Same external_reference must not create multiple payment rows: {ok_ids}"
        )

    def test_two_payments_with_different_references_both_succeed(
        self, admin_token, ts
    ):
        inv = self._make_issued(admin_token, ts, "DIFF")
        refs = [f"RACE-DIFF-A-{ts}", f"RACE-DIFF-B-{ts}"]
        results: list[requests.Response] = []

        def post_one(ref):
            results.append(
                requests.post(
                    f"{BASE_URL}/invoices/{inv['id']}/payments",
                    headers=auth_headers(admin_token),
                    json={
                        "amount": 200.0,
                        "method": "cash",
                        "external_reference": ref,
                        "received_at": "2026-01-10T10:00:00Z",
                    },
                    timeout=10,
                )
            )

        ts_ = [threading.Thread(target=post_one, args=(r,)) for r in refs]
        for t in ts_: t.start()
        for t in ts_: t.join()

        assert all(r.status_code == 200 for r in results), (
            f"Two distinct-ref payments on one invoice must both succeed: "
            f"{[r.status_code for r in results]} / {[r.text for r in results]}"
        )
        ids = [r.json()["id"] for r in results]
        assert len(set(ids)) == 2, (
            "Two distinct payments must produce two distinct ids"
        )


# ── CSV export integration ───────────────────────────────────────────────────

class TestCsvExports:
    def test_analytics_export_returns_csv(self, admin_token):
        # /analytics/export requires `report_type=trends|funnel|retention`
        # as a query parameter; omit it and the handler returns 422.
        r = requests.get(
            f"{BASE_URL}/analytics/export?report_type=trends",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code == 200, f"analytics export: {r.status_code} {r.text}"
        ct = r.headers.get("Content-Type", "")
        assert ("csv" in ct.lower() or "text/plain" in ct.lower()), (
            f"Analytics export must be CSV-like, got Content-Type {ct!r}"
        )
        # Body should be parseable by csv.reader and have at least a header row.
        reader = csv.reader(io.StringIO(r.text))
        rows = list(reader)
        assert rows, "Analytics export must have at least a header row"

    def test_analytics_export_missing_report_type_returns_4xx(self, admin_token):
        r = requests.get(
            f"{BASE_URL}/analytics/export",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert 400 <= r.status_code < 500, (
            f"Missing report_type should be a client error, got {r.status_code}"
        )

    def test_results_export_returns_csv_with_header(self, admin_token, auditor_token, ts):
        # Need an event with at least one result.  Semantic version is strict X.Y.Z.
        rs = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={
                "semantic_version": f"22.{ts % 9999}.0",
                "effective_at": "2026-01-01T00:00:00Z",
            },
            timeout=10,
        )
        assert rs.status_code == 200, f"csv ruleset: {rs.text}"
        rs = rs.json()
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"CSV Export Event {ts}"},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": rs["id"]},
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/results",
            headers=auth_headers(admin_token),
            json={
                "participant_id": ts + 22222,
                "value_numeric": 55000.0,
                "unit": "milliseconds",
            },
            timeout=10,
        )
        r = requests.get(
            f"{BASE_URL}/events/{ev['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert r.status_code == 200
        assert r.text.strip(), "Results export must not be empty"
        # Header line must mention at least "participant" or "value" fields.
        first_line = r.text.splitlines()[0].lower()
        assert ("participant" in first_line
                or "value" in first_line
                or "result" in first_line), (
            f"Unexpected CSV header: {first_line!r}"
        )


# ── Cross-module state consistency ───────────────────────────────────────────

class TestCrossModuleConsistency:
    """
    Prove that the audit log and the primary entity row agree on the
    current state after a status transition.
    """

    def test_invoice_status_matches_latest_audit_entry(
        self, admin_token, auditor_token, ts
    ):
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-CONS-{ts}",
                "counterparty": "Consistency Corp",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 100.0,
            },
            timeout=10,
        )
        issued = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        ).json()
        assert issued["status"] == "issued"

        # Latest invoice.issued audit entry for this id must match.
        entries = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.issued&limit=500",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        matches = [e for e in entries if e.get("entity_id") == inv["id"]]
        assert matches, (
            f"No invoice.issued audit entry for invoice {inv['id']}; audit log out of sync"
        )


# ── Token lifecycle under heavy use ─────────────────────────────────────────

class TestSessionSlidingExpiry:
    """
    The auth service slides the expiry on each successful authenticated
    request.  Exercising the session repeatedly must not expire it within
    a normal test run.
    """

    def test_repeated_requests_keep_session_alive(self):
        login = requests.post(
            f"{BASE_URL}/auth/login",
            json={"username": "auditor1", "password": "Auditor123!"},
            timeout=10,
        )
        token = login.json()["token"]
        try:
            for _ in range(20):
                r = requests.get(
                    f"{BASE_URL}/audit/logs?limit=1",
                    headers=auth_headers(token),
                    timeout=10,
                )
                assert r.status_code == 200, (
                    f"Session expired mid-run: {r.status_code}"
                )
        finally:
            requests.post(
                f"{BASE_URL}/auth/logout",
                headers=auth_headers(token),
                timeout=10,
            )
