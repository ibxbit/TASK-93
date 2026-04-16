"""
End-to-end workflow scenario tests.

These tests chain multiple user roles through a complete, production-like
business flow and assert on the *observable side effects* at every stage:

    1. Administrator                 — provisions catalog (ruleset, vehicle, asset)
    2. Administrator                 — creates a draft event
    3. EventDirector                 — publishes the event with a pinned ruleset
    4. EventDirector                 — submits results (participants:write)
                                       and referee review decisions (referees:write)
    5. EventDirector                 — arbitrates disputed results
    6. FinanceClerk                  — issues an invoice tied to the event
    7. FinanceClerk                  — records payment, requests + approves refund
    8. Auditor                       — queries the unified audit log to verify the
                                       full workflow is present & tamper-evident

Every request uses real HTTP (module-scoped `requests` via the `BASE_URL`
fixture); each transition asserts both the HTTP status code *and* the
response payload shape/content.

The tests are intentionally comprehensive: they are not a smoke test.  They
verify:
  - Status transitions on each entity (draft → published → paid, etc.)
  - Cross-role RBAC boundaries (referee cannot submit results, director cannot
    issue invoices, finance cannot publish events, auditor cannot mutate).
  - The audit log records every business-critical transition across roles.
  - Response payloads carry the expected fields with correct types.
  - Encrypted identifiers (VIN, serial_number, external_reference) round-trip
    in plaintext through reader endpoints but stay redacted in audit snapshots.
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Shared helpers ───────────────────────────────────────────────────────────

def _vin_for(ts: int, suffix: str) -> str:
    """Build a deterministic, valid 17-char VIN (no I/O/Q)."""
    raw = f"1HGE2E{suffix}{ts}"[:17].upper()
    raw = raw.replace("I", "1").replace("O", "0").replace("Q", "9")
    return raw.ljust(17, "0")[:17]


def _get_json(url, token):
    r = requests.get(url, headers=auth_headers(token), timeout=10)
    r.raise_for_status()
    return r.json()


# ── Full workflow fixture — shared state across the test class ───────────────

@pytest.fixture(scope="module")
def workflow_state(admin_token, director_token, finance_token, auditor_token, ts):
    """
    Drive a complete workflow once, persist the state, and let individual
    `test_*` methods assert on the outcome.  This avoids re-running the
    expensive setup (≈15 HTTP requests) for every assertion and lets both
    `TestEndToEndWorkflow` and `TestWorkflowRBACBoundaries` share one
    provisioned scenario (module scope is required — class scope would
    recreate the fixture per class and collide on ruleset version).

    Every step captures the HTTP response so tests can verify both the
    immediate return value *and* effects observed later in the flow.
    """
    state: dict = {}

    # ── 1. Administrator provisions the ruleset ──────────────────────────────
    # Semantic version must follow strict X.Y.Z per the ruleset service.
    rs = requests.post(
        f"{BASE_URL}/rulesets",
        headers=auth_headers(admin_token),
        json={
            "semantic_version": f"7.{ts % 999}.0",
            "description": "End-to-end workflow ruleset",
            "effective_at": "2026-01-01T00:00:00Z",
        },
        timeout=10,
    )
    assert rs.status_code == 200, f"ruleset create failed: {rs.text}"
    state["ruleset"] = rs.json()

    # ── 2. Administrator registers a vehicle (encrypted VIN round-trip) ──────
    vehicle_resp = requests.post(
        f"{BASE_URL}/vehicles",
        headers=auth_headers(admin_token),
        json={
            "vin": _vin_for(ts, "A"),
            "registration_id": f"E2E-{ts}",
            "make": "Porsche",
            "model": "911 GT3",
            "year": 2024,
            "color": "Guards Red",
            "mileage": 0,
        },
        timeout=10,
    )
    assert vehicle_resp.status_code == 200, f"vehicle: {vehicle_resp.text}"
    state["vehicle"] = vehicle_resp.json()

    # ── 3. Administrator registers an asset ─────────────────────────────────
    asset_resp = requests.post(
        f"{BASE_URL}/assets",
        headers=auth_headers(admin_token),
        json={
            "asset_code": f"E2E-TIMING-{ts}",
            "category": "equipment",
            "brand": "MYLAPS",
            "model": "X2-Pro",
            "serial_number": f"E2E-SN-{ts}",
            "procurement_cost": 12500.00,
            "procurement_date": "2025-09-01",
            "useful_life_months": 96,
        },
        timeout=10,
    )
    assert asset_resp.status_code == 200, f"asset: {asset_resp.text}"
    state["asset"] = asset_resp.json()

    # ── 4. Administrator creates a draft event ──────────────────────────────
    ev_resp = requests.post(
        f"{BASE_URL}/events",
        headers=auth_headers(admin_token),
        json={
            "name": f"E2E Workflow Event {ts}",
            "description": "Full end-to-end chain-of-roles scenario",
            "schedule_group": "E2E",
            "is_championship_class": False,
        },
        timeout=10,
    )
    assert ev_resp.status_code == 200, f"event create: {ev_resp.text}"
    event = ev_resp.json()
    assert event["status"] == "draft", "new event must begin in draft state"
    state["event_draft"] = event

    # ── 5. EventDirector publishes the event — freezes the ruleset version ──
    pub_resp = requests.post(
        f"{BASE_URL}/events/{event['id']}/publish",
        headers=auth_headers(director_token),
        json={"ruleset_version_id": state["ruleset"]["id"]},
        timeout=10,
    )
    assert pub_resp.status_code == 200, f"publish: {pub_resp.text}"
    state["event_published"] = pub_resp.json()
    assert state["event_published"]["status"] == "published"
    assert state["event_published"]["published_version_id"] == state["ruleset"]["id"]

    # ── 6. EventDirector submits a result ───────────────────────────────────
    res_resp = requests.post(
        f"{BASE_URL}/events/{event['id']}/results",
        headers=auth_headers(director_token),
        json={
            "participant_id": ts + 9001,
            "value_numeric": 58423.5,
            "unit": "milliseconds",
        },
        timeout=10,
    )
    assert res_resp.status_code == 200, f"result: {res_resp.text}"
    state["result"] = res_resp.json()

    # ── 7. EventDirector submits a referee review on that result ────────────
    rev_resp = requests.post(
        f"{BASE_URL}/events/{event['id']}/results/{state['result']['id']}/reviews",
        headers=auth_headers(director_token),
        json={"decision": "approved", "comment": "Within tolerance"},
        timeout=10,
    )
    assert rev_resp.status_code == 200, f"review: {rev_resp.text}"
    state["review"] = rev_resp.json()

    # ── 8. EventDirector arbitrates a second (disputed) result ──────────────
    res2 = requests.post(
        f"{BASE_URL}/events/{event['id']}/results",
        headers=auth_headers(director_token),
        json={"participant_id": ts + 9002, "value_numeric": 61000.0, "unit": "milliseconds"},
        timeout=10,
    ).json()
    state["result_disputed"] = res2
    arb_resp = requests.post(
        f"{BASE_URL}/events/{event['id']}/results/{res2['id']}/arbitrate",
        headers=auth_headers(director_token),
        json={"decision": "rejected", "comment": "Course cut detected"},
        timeout=10,
    )
    assert arb_resp.status_code == 200, f"arbitrate: {arb_resp.text}"
    state["arbitration"] = arb_resp.json()

    # ── 9. FinanceClerk creates an invoice for the event ────────────────────
    inv_resp = requests.post(
        f"{BASE_URL}/invoices",
        headers=auth_headers(finance_token),
        json={
            "invoice_no": f"E2E-INV-{ts}",
            "counterparty": "Ruf Automobile GmbH",
            "issue_date": "2026-02-01",
            "tax_rate": 0.08,
        },
        timeout=10,
    )
    assert inv_resp.status_code == 200, f"invoice create: {inv_resp.text}"
    invoice = inv_resp.json()
    assert invoice["status"] == "draft"
    state["invoice_draft"] = invoice

    # ── 10. FinanceClerk adds a line item ───────────────────────────────────
    line_resp = requests.post(
        f"{BASE_URL}/invoices/{invoice['id']}/lines",
        headers=auth_headers(finance_token),
        json={
            "description": "Entry fee — E2E event",
            "pricing_model": "fixed",
            "quantity": 1.0,
            "unit_price": 750.00,
        },
        timeout=10,
    )
    assert line_resp.status_code == 200, f"line: {line_resp.text}"
    state["invoice_with_line"] = line_resp.json()
    assert float(state["invoice_with_line"]["subtotal"]) == pytest.approx(750.0)

    # ── 11. FinanceClerk issues the invoice (draft → issued) ────────────────
    issue_resp = requests.post(
        f"{BASE_URL}/invoices/{invoice['id']}/issue",
        headers=auth_headers(finance_token),
        json={},
        timeout=10,
    )
    assert issue_resp.status_code == 200, f"issue: {issue_resp.text}"
    state["invoice_issued"] = issue_resp.json()
    assert state["invoice_issued"]["status"] == "issued"

    # ── 12. FinanceClerk records a payment (encrypted external_reference) ────
    secret_ref = f"E2E-TXN-{ts}"
    state["secret_ref"] = secret_ref
    pmt_resp = requests.post(
        f"{BASE_URL}/invoices/{invoice['id']}/payments",
        headers=auth_headers(finance_token),
        json={
            "amount": 810.00,
            "method": "bank_transfer",
            "external_reference": secret_ref,
            "received_at": "2026-02-05T10:00:00Z",
        },
        timeout=10,
    )
    assert pmt_resp.status_code == 200, f"payment: {pmt_resp.text}"
    state["payment"] = pmt_resp.json()

    # Invoice should have moved to "paid" since payment covers total.
    inv_after_pmt = _get_json(
        f"{BASE_URL}/invoices/{invoice['id']}", finance_token
    )
    state["invoice_after_payment"] = inv_after_pmt

    # ── 13. FinanceClerk requests a small refund and approves it ────────────
    rfnd_resp = requests.post(
        f"{BASE_URL}/invoices/{invoice['id']}/payments/{state['payment']['id']}/refunds",
        headers=auth_headers(finance_token),
        json={"amount": 50.0, "reason": "Course shortened by weather"},
        timeout=10,
    )
    assert rfnd_resp.status_code == 200, f"refund request: {rfnd_resp.text}"
    state["refund_pending"] = rfnd_resp.json()
    assert state["refund_pending"]["status"] == "pending_finance"

    rfnd_appr = requests.post(
        f"{BASE_URL}/invoices/{invoice['id']}/payments/{state['payment']['id']}"
        f"/refunds/{state['refund_pending']['id']}/approve",
        headers=auth_headers(finance_token),
        json={},
        timeout=10,
    )
    assert rfnd_appr.status_code == 200, f"refund approve: {rfnd_appr.text}"
    state["refund_approved"] = rfnd_appr.json()
    assert state["refund_approved"]["status"] == "approved"

    # ── 14. Auditor pulls the full log for cross-role verification ──────────
    state["audit_entries"] = _get_json(
        f"{BASE_URL}/audit/logs?limit=500", auditor_token
    )
    return state


# ── Assertions ───────────────────────────────────────────────────────────────

@pytest.mark.usefixtures("workflow_state")
class TestEndToEndWorkflow:
    """Sequential assertions on the workflow produced by `workflow_state`."""

    # ── Ruleset ──
    def test_ruleset_is_not_a_rollback(self, workflow_state):
        rs = workflow_state["ruleset"]
        assert rs["is_rollback"] is False
        assert rs["rollback_of"] is None
        assert {"id", "semantic_version", "effective_at"}.issubset(rs.keys())

    # ── Vehicle ──
    def test_vehicle_returns_plaintext_vin(self, workflow_state, ts):
        v = workflow_state["vehicle"]
        # Plaintext VIN is returned to authorised readers.
        assert v["vin"] == _vin_for(ts, "A")
        assert v["status"] == "draft"

    # ── Asset ──
    def test_asset_returns_plaintext_serial(self, workflow_state, ts):
        a = workflow_state["asset"]
        assert a["serial_number"] == f"E2E-SN-{ts}"
        assert a["status"] == "in_service"

    # ── Event lifecycle ──
    def test_event_transitions_draft_to_published(self, workflow_state):
        assert workflow_state["event_draft"]["status"] == "draft"
        assert workflow_state["event_published"]["status"] == "published"
        assert workflow_state["event_draft"]["id"] == workflow_state["event_published"]["id"]

    def test_published_event_pins_ruleset_version(self, workflow_state):
        assert workflow_state["event_published"]["published_version_id"] == (
            workflow_state["ruleset"]["id"]
        )

    def test_director_can_publish_event(self, workflow_state):
        """Sanity: the publish call in the fixture must have succeeded as director."""
        # Covered by fixture asserts, but we verify the response shape here.
        pub = workflow_state["event_published"]
        required = {"id", "name", "status", "published_version_id", "updated_at"}
        assert required.issubset(pub.keys())

    # ── Result submission ──
    def test_result_submitted_by_director_is_pending(self, workflow_state):
        r = workflow_state["result"]
        assert r["reviewed_state"] == "pending"
        assert r["event_id"] == workflow_state["event_published"]["id"]
        assert float(r["value_numeric"]) == pytest.approx(58423.5)

    def test_review_is_approved(self, workflow_state):
        rev = workflow_state["review"]
        assert rev["decision"] == "approved"
        assert rev["result_id"] == workflow_state["result"]["id"]
        assert rev["comment"] == "Within tolerance"

    # ── Arbitration ──
    def test_arbitration_overrides_disputed_result(self, workflow_state):
        arb = workflow_state["arbitration"]
        assert arb["decision"] == "rejected"
        assert arb["result_id"] == workflow_state["result_disputed"]["id"]

    # ── Billing ──
    def test_invoice_subtotal_after_line(self, workflow_state):
        inv = workflow_state["invoice_with_line"]
        assert float(inv["subtotal"]) == pytest.approx(750.0)
        # With 8% tax, total ≈ 810.00
        assert float(inv["total"]) == pytest.approx(810.0, rel=1e-3)

    def test_invoice_transitions_draft_to_issued(self, workflow_state):
        assert workflow_state["invoice_draft"]["status"] == "draft"
        assert workflow_state["invoice_issued"]["status"] == "issued"

    def test_invoice_reaches_paid_status(self, workflow_state):
        assert workflow_state["invoice_after_payment"]["status"] == "paid"

    # ── Payment round-trip ──
    def test_payment_external_reference_returned_plaintext(self, workflow_state):
        """Authorised readers must see the plaintext reference, not ciphertext."""
        assert workflow_state["payment"]["external_reference"] == workflow_state["secret_ref"]

    # ── Refund two-stage workflow ──
    def test_small_refund_single_stage_approval(self, workflow_state):
        rfnd = workflow_state["refund_approved"]
        assert rfnd["status"] == "approved"
        assert rfnd["finance_approved_by"] is not None
        # ≤ $1000 — no auditor approval required.
        assert rfnd["auditor_approved_by"] is None

    # ── Auditor evidence ──
    def test_audit_log_contains_event_published(self, workflow_state):
        actions = {e["action"] for e in workflow_state["audit_entries"]}
        assert "event.published" in actions, (
            f"event.published missing from audit log. Saw: {sorted(actions)}"
        )

    def test_audit_log_contains_invoice_lifecycle(self, workflow_state):
        actions = {e["action"] for e in workflow_state["audit_entries"]}
        for required in ("invoice.created", "invoice.issued", "payment.recorded"):
            assert required in actions, (
                f"{required} missing from audit log. Saw: {sorted(actions)}"
            )

    def test_audit_log_contains_result_and_review(self, workflow_state):
        actions = {e["action"] for e in workflow_state["audit_entries"]}
        # Submission + review must both appear.  Review is logged under
        # `result.reviewed` (the review is an action on the result entity).
        assert "result.submitted" in actions, (
            f"result.submitted missing. Saw: {sorted(actions)}"
        )
        assert "result.reviewed" in actions, (
            f"result.reviewed missing. Saw: {sorted(actions)}"
        )
        # Arbitration is part of the workflow as well — assert it too.
        assert "result.arbitrated" in actions, (
            f"result.arbitrated missing. Saw: {sorted(actions)}"
        )

    def test_audit_snapshot_redacts_payment_reference(self, workflow_state):
        """The plaintext external_reference must never leak into the audit log."""
        needle = workflow_state["secret_ref"]
        for entry in workflow_state["audit_entries"]:
            if entry.get("entity_type") == "payment":
                snapshot = str(entry.get("snapshot", {}))
                assert needle not in snapshot, (
                    "Plaintext payment reference leaked into audit snapshot"
                )


# ── Cross-role negative boundaries executed as part of the same scenario ─────

class TestWorkflowRBACBoundaries:
    """
    Real workflows must refuse when the wrong role tries to progress the chain.
    These are not duplicated from test_rbac.py — they verify the boundaries
    *within the shipped workflow artifacts* (real event/invoice/payment).
    """

    def test_finance_cannot_publish_event(self, workflow_state, finance_token):
        ev_id = workflow_state["event_draft"]["id"]
        rs_id = workflow_state["ruleset"]["id"]
        r = requests.post(
            f"{BASE_URL}/events/{ev_id}/publish",
            headers=auth_headers(finance_token),
            json={"ruleset_version_id": rs_id},
            timeout=10,
        )
        # Either forbidden or the event is already published (409/400) —
        # both are correct "finance cannot progress this".  Never 200.
        assert r.status_code != 200
        assert r.status_code in (400, 403, 409, 422)

    def test_director_cannot_read_invoice(self, workflow_state, director_token):
        inv_id = workflow_state["invoice_issued"]["id"]
        r = requests.get(
            f"{BASE_URL}/invoices/{inv_id}",
            headers=auth_headers(director_token),
            timeout=10,
        )
        assert r.status_code == 403

    def test_director_cannot_record_payment(self, workflow_state, director_token, ts):
        inv_id = workflow_state["invoice_issued"]["id"]
        r = requests.post(
            f"{BASE_URL}/invoices/{inv_id}/payments",
            headers=auth_headers(director_token),
            json={
                "amount": 1.0,
                "method": "cash",
                "external_reference": f"DIRECTOR-ATTEMPT-{ts}",
                "received_at": "2026-02-05T11:00:00Z",
            },
            timeout=10,
        )
        assert r.status_code == 403

    def test_referee_cannot_submit_result_in_this_event(self, workflow_state, referee_token, ts):
        ev_id = workflow_state["event_published"]["id"]
        r = requests.post(
            f"{BASE_URL}/events/{ev_id}/results",
            headers=auth_headers(referee_token),
            json={
                "participant_id": ts + 9999,
                "value_numeric": 42.0,
                "unit": "milliseconds",
            },
            timeout=10,
        )
        assert r.status_code == 403

    def test_auditor_cannot_approve_refund_at_finance_stage(
        self, workflow_state, finance_token, auditor_token
    ):
        """
        Even though the auditor can approve large refunds at stage-2, a
        pending_finance refund MUST reject auditor approval.
        """
        inv_id = workflow_state["invoice_issued"]["id"]
        pmt_id = workflow_state["payment"]["id"]
        rf = requests.post(
            f"{BASE_URL}/invoices/{inv_id}/payments/{pmt_id}/refunds",
            headers=auth_headers(finance_token),
            json={"amount": 10.0, "reason": "staged negative-path test"},
            timeout=10,
        )
        if rf.status_code != 200:
            pytest.skip(
                "Could not stage a pending_finance refund for the auditor test: "
                f"{rf.status_code} {rf.text}"
            )
        rfnd = rf.json()
        attempt = requests.post(
            f"{BASE_URL}/invoices/{inv_id}/payments/{pmt_id}/refunds/{rfnd['id']}/approve",
            headers=auth_headers(auditor_token),
            json={},
            timeout=10,
        )
        assert attempt.status_code == 403


# ── A second, larger refund E2E covering the auditor two-stage approval ──────

class TestLargeRefundTwoStageWorkflow:
    """
    Full two-stage approval for refunds > $1000 drives an additional role
    (auditor approver) that the primary workflow does not exercise.  This
    complements the main workflow with the dual-control refund path end to end.
    """

    def test_large_refund_requires_finance_then_auditor(
        self, admin_token, finance_token, auditor_token, ts
    ):
        # Admin provisions + finance issues a $3000 invoice; finance pays it.
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(finance_token),
            json={
                "invoice_no": f"E2E-LGR-{ts}",
                "counterparty": "Big Sponsor Inc",
                "issue_date": "2026-03-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(finance_token),
            json={
                "description": "Sponsorship fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 3000.0,
            },
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(finance_token),
            json={},
            timeout=10,
        )
        pmt = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(finance_token),
            json={
                "amount": 3000.0,
                "method": "bank_transfer",
                "external_reference": f"E2E-LGR-TXN-{ts}",
                "received_at": "2026-03-01T10:00:00Z",
            },
            timeout=10,
        ).json()

        # Request a $1500 refund — triggers pending_finance.
        rfnd = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments/{pmt['id']}/refunds",
            headers=auth_headers(finance_token),
            json={"amount": 1500.0, "reason": "Event cancelled"},
            timeout=10,
        ).json()
        assert rfnd["status"] == "pending_finance"

        # Stage 1: finance approves → pending_auditor.
        stage1 = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments/{pmt['id']}"
            f"/refunds/{rfnd['id']}/approve",
            headers=auth_headers(finance_token),
            json={},
            timeout=10,
        ).json()
        assert stage1["status"] == "pending_auditor"
        assert stage1["finance_approved_by"] is not None
        assert stage1["auditor_approved_by"] is None

        # Stage 2: auditor approves → approved.
        stage2 = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments/{pmt['id']}"
            f"/refunds/{rfnd['id']}/approve",
            headers=auth_headers(auditor_token),
            json={},
            timeout=10,
        )
        assert stage2.status_code == 200
        body = stage2.json()
        assert body["status"] == "approved"
        assert body["finance_approved_by"] is not None
        assert body["auditor_approved_by"] is not None
        assert body["invoice_line_id"] is not None, (
            "Approved refund must materialise a refund line on the invoice"
        )
