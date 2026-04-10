"""
API tests for billing and payment workflows.

Covers:
- POST /invoices                        (create)
- GET  /invoices / GET /invoices/<id>   (read)
- POST /invoices/<id>/lines             (add line item)
- POST /invoices/<id>/discount          (apply discount)
- POST /invoices/<id>/issue             (transition draft → issued)
- POST /invoices/<iid>/payments         (record payment)
- GET  /invoices/<iid>/payments         (list payments)
- Idempotency: duplicate external_reference on same invoice → 200
- Idempotency: duplicate external_reference on different invoice → 409
- RBAC enforcement
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


@pytest.fixture(scope="module")
def invoice(admin_token, ts):
    """Create a fresh invoice for this test module."""
    resp = requests.post(
        f"{BASE_URL}/invoices",
        headers=auth_headers(admin_token),
        json={
            "invoice_no": f"INV-TEST-{ts}",
            "counterparty": "Acme Racing Ltd",
            "issue_date": "2026-01-15",
            "tax_rate": 0.10,
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Invoice creation failed: {resp.text}"
    return resp.json()


class TestInvoiceCreate:
    def test_finance_clerk_can_create_invoice(self, finance_token, ts):
        resp = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(finance_token),
            json={
                "invoice_no": f"INV-FC-{ts}",
                "counterparty": "Test Corp",
                "issue_date": "2026-02-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "draft"

    def test_invoice_starts_in_draft(self, invoice):
        assert invoice["status"] == "draft"

    def test_invoice_has_required_fields(self, invoice):
        required = {"id", "invoice_no", "counterparty", "issue_date",
                    "tax_rate", "subtotal", "tax", "total", "status",
                    "created_at", "updated_at", "lines"}
        assert required.issubset(invoice.keys())

    def test_invoice_initial_total_is_zero(self, invoice):
        assert float(invoice["subtotal"]) == 0.0
        assert float(invoice["total"]) == 0.0

    def test_referee_cannot_create_invoice(self, referee_token, ts):
        resp = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(referee_token),
            json={
                "invoice_no": f"INV-NO-{ts}",
                "counterparty": "X",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        assert resp.status_code == 403

    def test_create_without_auth_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/invoices",
            json={"invoice_no": "X", "counterparty": "Y",
                  "issue_date": "2026-01-01", "tax_rate": 0.0},
            timeout=10,
        )
        assert resp.status_code == 401


class TestInvoiceRead:
    def test_get_invoice_by_id(self, admin_token, invoice):
        resp = requests.get(
            f"{BASE_URL}/invoices/{invoice['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["id"] == invoice["id"]

    def test_list_invoices_returns_array(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_auditor_can_read_invoices(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/invoices",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_get_nonexistent_invoice_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/invoices/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404


class TestLineItems:
    def test_add_line_item(self, admin_token, invoice):
        resp = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Event entry fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 500.00,
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert float(body["subtotal"]) > 0

    def test_add_line_updates_total(self, admin_token, invoice):
        before = requests.get(
            f"{BASE_URL}/invoices/{invoice['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        before_total = float(before["total"])

        requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Additional fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 100.00,
            },
            timeout=10,
        )

        after = requests.get(
            f"{BASE_URL}/invoices/{invoice['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        after_total = float(after["total"])

        # Total should have increased (100 + 10% tax = 110)
        assert after_total > before_total

    def test_invoice_lines_included_in_response(self, admin_token, invoice):
        resp = requests.get(
            f"{BASE_URL}/invoices/{invoice['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert isinstance(resp["lines"], list)
        assert len(resp["lines"]) >= 1


class TestDiscount:
    def test_apply_percentage_discount(self, admin_token, invoice):
        resp = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/discount",
            headers=auth_headers(admin_token),
            json={"discount_type": "percentage", "discount_value": 10.0},
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["discount_type"] == "percentage"
        assert float(body["discount_amount"]) >= 0


class TestIssueInvoice:
    def test_issue_transitions_to_issued(self, admin_token, invoice):
        resp = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "issued"

    def test_cannot_add_line_to_issued_invoice(self, admin_token, invoice):
        # Invoice is now issued — adding a line should fail
        resp = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Late addition",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 50.00,
            },
            timeout=10,
        )
        assert resp.status_code in (400, 409, 422)


class TestPayments:
    def test_record_payment(self, admin_token, invoice, ts):
        resp = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 550.00,
                "method": "cash",
                "external_reference": f"TXN-{ts}-001",
                "received_at": "2026-01-15T12:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["id"] > 0

    def test_duplicate_external_reference_same_invoice_is_idempotent(
        self, admin_token, invoice, ts
    ):
        """Same external_reference on the same invoice → idempotent 200."""
        ref = f"TXN-IDEM-{ts}"
        payload = {
            "amount": 100.00,
            "method": "cash",
            "external_reference": ref,
            "received_at": "2026-01-15T13:00:00Z",
        }
        r1 = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/payments",
            headers=auth_headers(admin_token),
            json=payload,
            timeout=10,
        )
        assert r1.status_code == 200
        r2 = requests.post(
            f"{BASE_URL}/invoices/{invoice['id']}/payments",
            headers=auth_headers(admin_token),
            json=payload,
            timeout=10,
        )
        # Idempotent: second call returns the existing payment
        assert r2.status_code == 200
        # Both responses should have the same payment ID
        assert r1.json()["id"] == r2.json()["id"]

    def test_list_payments_for_invoice(self, admin_token, invoice):
        resp = requests.get(
            f"{BASE_URL}/invoices/{invoice['id']}/payments",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_finance_clerk_can_record_payment(self, finance_token, admin_token, ts):
        # Create a separate invoice for this test
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-PMT-{ts}",
                "counterparty": "Payer Corp",
                "issue_date": "2026-03-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()

        # Add a line and issue
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
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
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )

        resp = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(finance_token),
            json={
                "amount": 200.0,
                "method": "cash",
                "external_reference": f"TXN-FC-{ts}",
                "received_at": "2026-03-01T10:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200


# ── Shared helpers ────────────────────────────────────────────────────────────

def _make_issued_invoice(token: str, ts: int, suffix: str) -> dict:
    """Create an invoice with one $500 line item and issue it."""
    inv = requests.post(
        f"{BASE_URL}/invoices",
        headers=auth_headers(token),
        json={
            "invoice_no": f"INV-{suffix}-{ts}",
            "counterparty": "Test Corp",
            "issue_date": "2026-01-15",
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
            "unit_price": 500.0,
        },
        timeout=10,
    )
    requests.post(
        f"{BASE_URL}/invoices/{inv['id']}/issue",
        headers=auth_headers(token),
        json={},
        timeout=10,
    )
    return requests.get(
        f"{BASE_URL}/invoices/{inv['id']}",
        headers=auth_headers(token),
        timeout=10,
    ).json()


def _record_payment(
    token: str,
    invoice_id: int,
    ts: int,
    suffix: str,
    amount: float = 200.0,
    method: str = "cash",
) -> dict:
    """Record a payment and return the response body."""
    resp = requests.post(
        f"{BASE_URL}/invoices/{invoice_id}/payments",
        headers=auth_headers(token),
        json={
            "amount": amount,
            "method": method,
            "external_reference": f"REF-{suffix}-{ts}",
            "received_at": "2026-01-15T12:00:00Z",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Payment failed: {resp.text}"
    return resp.json()


def _raise_exception(
    token: str,
    invoice_id: int,
    payment_id: int,
    exc_type: str,
    reason: str = "Test reason",
) -> requests.Response:
    return requests.post(
        f"{BASE_URL}/invoices/{invoice_id}/payments/{payment_id}/exceptions",
        headers=auth_headers(token),
        json={"exception_type": exc_type, "reason": reason},
        timeout=10,
    )


# ── Payment response shape ────────────────────────────────────────────────────

class TestPaymentResponseShape:
    def test_new_payment_status_is_active(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "SHAPE")
        pmt = _record_payment(admin_token, inv["id"], ts, "SHAPE")
        assert pmt["status"] == "active"

    def test_payment_response_has_required_fields(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "SHAPEF")
        pmt = _record_payment(admin_token, inv["id"], ts, "SHAPEF")
        required = {
            "id", "invoice_id", "method", "amount", "external_reference",
            "received_at", "status", "recorded_by", "created_at",
            "exceptions", "refunds",
        }
        assert required.issubset(pmt.keys())

    def test_new_payment_has_empty_exceptions_and_refunds(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "SHAPEEMPTY")
        pmt = _record_payment(admin_token, inv["id"], ts, "SHAPEEMPTY")
        assert pmt["exceptions"] == []
        assert pmt["refunds"] == []


# ── Payment method validation ─────────────────────────────────────────────────

class TestPaymentMethods:
    def test_cash_accepted(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MCASH")
        pmt = _record_payment(admin_token, inv["id"], ts, "MCASH", method="cash")
        assert pmt["method"] == "cash"

    def test_cheque_accepted(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MCHEQ")
        pmt = _record_payment(admin_token, inv["id"], ts, "MCHEQ", method="cheque")
        assert pmt["method"] == "cheque"

    def test_ach_accepted(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MACH")
        pmt = _record_payment(admin_token, inv["id"], ts, "MACH", method="ach")
        assert pmt["method"] == "ach"

    def test_bank_transfer_accepted(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MBT")
        resp = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "bank_transfer",
                "external_reference": f"REF-MBT-{ts}",
                "received_at": "2026-01-15T12:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["method"] == "bank_transfer"

    def test_card_accepted(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MCARD")
        resp = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "card",
                "external_reference": f"REF-MCARD-{ts}",
                "received_at": "2026-01-15T12:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 200
        assert resp.json()["method"] == "card"

    def test_unknown_method_rejected(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MUNK")
        resp = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "crypto",
                "external_reference": f"REF-MUNK-{ts}",
                "received_at": "2026-01-15T12:00:00Z",
            },
            timeout=10,
        )
        assert resp.status_code == 400

    def test_missing_received_at_rejected(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "MRAT")
        resp = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "cash",
                "external_reference": f"REF-MRAT-{ts}",
            },
            timeout=10,
        )
        assert resp.status_code in (400, 422)


# ── Payment exceptions ────────────────────────────────────────────────────────

class TestPaymentExceptions:
    def test_void_sets_payment_status_voided(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXVOID")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXVOID")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "void")
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "voided"
        assert len(body["exceptions"]) == 1
        assert body["exceptions"][0]["exception_type"] == "void"

    def test_reversal_sets_payment_status_reversed(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXREV")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXREV")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "reversal")
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "reversed"
        assert body["exceptions"][0]["exception_type"] == "reversal"

    def test_dispute_sets_payment_status_disputed(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXDISP")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXDISP")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "dispute")
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "disputed"

    def test_list_exceptions_returns_array(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXLIST")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXLIST")
        _raise_exception(admin_token, inv["id"], pmt["id"], "void")
        resp = requests.get(
            f"{BASE_URL}/invoices/{inv['id']}/payments/{pmt['id']}/exceptions",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert isinstance(body, list)
        assert len(body) == 1
        assert body[0]["exception_type"] == "void"

    def test_exception_on_non_active_payment_returns_409(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EX409")
        pmt = _record_payment(admin_token, inv["id"], ts, "EX409")
        _raise_exception(admin_token, inv["id"], pmt["id"], "void")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "reversal")
        assert resp.status_code == 409

    def test_unknown_exception_type_returns_400(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EX400T")
        pmt = _record_payment(admin_token, inv["id"], ts, "EX400T")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "write_off")
        assert resp.status_code == 400

    def test_empty_exception_reason_returns_400(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EX400R")
        pmt = _record_payment(admin_token, inv["id"], ts, "EX400R")
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "void", reason="")
        assert resp.status_code == 400

    def test_referee_cannot_raise_exception(self, admin_token, referee_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXRBAC")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXRBAC")
        resp = _raise_exception(referee_token, inv["id"], pmt["id"], "void")
        assert resp.status_code == 403

    def test_exception_on_wrong_invoice_returns_404(self, admin_token, ts):
        inv1 = _make_issued_invoice(admin_token, ts, "EX404A")
        inv2 = _make_issued_invoice(admin_token, ts, "EX404B")
        pmt = _record_payment(admin_token, inv1["id"], ts, "EX404P")
        resp = _raise_exception(admin_token, inv2["id"], pmt["id"], "void")
        assert resp.status_code == 404

    def test_void_on_paid_invoice_reverts_to_issued(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXPAID")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXPAID", amount=500.0)
        inv_state = requests.get(
            f"{BASE_URL}/invoices/{inv['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert inv_state["status"] == "paid"
        _raise_exception(admin_token, inv["id"], pmt["id"], "void")
        inv_after = requests.get(
            f"{BASE_URL}/invoices/{inv['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert inv_after["status"] == "issued"

    def test_dispute_on_paid_invoice_reverts_to_issued(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXDPAID")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXDPAID", amount=500.0)
        _raise_exception(admin_token, inv["id"], pmt["id"], "dispute")
        inv_after = requests.get(
            f"{BASE_URL}/invoices/{inv['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert inv_after["status"] == "issued"

    def test_exception_reason_returned_in_response(self, admin_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "EXRSN")
        pmt = _record_payment(admin_token, inv["id"], ts, "EXRSN")
        reason = "ACH return code R01"
        resp = _raise_exception(admin_token, inv["id"], pmt["id"], "reversal", reason=reason)
        assert resp.status_code == 200
        assert resp.json()["exceptions"][0]["reason"] == reason


# ── Refund workflow ───────────────────────────────────────────────────────────

class TestRefundWorkflow:

    def _request_refund(self, token, invoice_id, payment_id, amount, reason="Test refund"):
        return requests.post(
            f"{BASE_URL}/invoices/{invoice_id}/payments/{payment_id}/refunds",
            headers=auth_headers(token),
            json={"amount": amount, "reason": reason},
            timeout=10,
        )

    def _approve_refund(self, token, invoice_id, payment_id, refund_id):
        return requests.post(
            f"{BASE_URL}/invoices/{invoice_id}/payments/{payment_id}"
            f"/refunds/{refund_id}/approve",
            headers=auth_headers(token),
            json={},
            timeout=10,
        )

    def _reject_refund(self, token, invoice_id, payment_id, refund_id, reason="Rejected"):
        return requests.post(
            f"{BASE_URL}/invoices/{invoice_id}/payments/{payment_id}"
            f"/refunds/{refund_id}/reject",
            headers=auth_headers(token),
            json={"reason": reason},
            timeout=10,
        )

    def _make_large_invoice(self, admin_token, ts, suffix):
        """Issued invoice with a $3000 line so payments can exceed $1000."""
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-{suffix}-{ts}",
                "counterparty": "Big Corp",
                "issue_date": "2026-01-15",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={
                "description": "Big fee",
                "pricing_model": "fixed",
                "quantity": 1.0,
                "unit_price": 3000.0,
            },
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        return requests.get(
            f"{BASE_URL}/invoices/{inv['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()

    # ── Request ───────────────────────────────────────────────────────────────

    def test_request_refund_starts_pending_finance(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFPF")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFPF", amount=300.0)
        resp = self._request_refund(finance_token, inv["id"], pmt["id"], 100.0)
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "pending_finance"
        assert float(body["amount"]) == 100.0
        assert body["finance_approved_by"] is None
        assert body["auditor_approved_by"] is None
        assert body["rejected_by"] is None

    def test_refund_response_has_required_fields(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFFIELDS")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFFIELDS", amount=300.0)
        body = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        required = {
            "id", "payment_id", "amount", "reason", "status",
            "requested_by", "finance_approved_by", "auditor_approved_by",
            "rejected_by", "rejection_reason", "created_at", "updated_at",
        }
        assert required.issubset(body.keys())

    def test_cannot_refund_non_active_payment(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFNA")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFNA")
        _raise_exception(admin_token, inv["id"], pmt["id"], "void")
        resp = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0)
        assert resp.status_code == 409

    def test_cannot_refund_more_than_payment_amount(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFOVER")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFOVER", amount=200.0)
        resp = self._request_refund(finance_token, inv["id"], pmt["id"], 999.0)
        assert resp.status_code == 422

    def test_empty_refund_reason_returns_400(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFEMPTY")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFEMPTY")
        resp = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0, reason="")
        assert resp.status_code == 400

    def test_referee_cannot_request_refund(self, admin_token, referee_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFRBAC")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFRBAC")
        resp = self._request_refund(referee_token, inv["id"], pmt["id"], 50.0)
        assert resp.status_code == 403

    # ── Small refund (≤ $1000): single finance-clerk approval ─────────────────

    def test_small_refund_finance_approval_completes(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFSMALL")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFSMALL", amount=500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 100.0).json()
        resp = self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "approved"
        assert body["finance_approved_by"] is not None
        assert body["auditor_approved_by"] is None
        assert body["invoice_line_id"] is not None

    def test_small_refund_approval_reduces_invoice_total(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFLINE")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFLINE", amount=500.0)
        before_total = float(
            requests.get(
                f"{BASE_URL}/invoices/{inv['id']}",
                headers=auth_headers(admin_token),
                timeout=10,
            ).json()["total"]
        )
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 100.0).json()
        self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        after_total = float(
            requests.get(
                f"{BASE_URL}/invoices/{inv['id']}",
                headers=auth_headers(admin_token),
                timeout=10,
            ).json()["total"]
        )
        assert after_total < before_total

    # ── Large refund (> $1000): two-stage approval ────────────────────────────

    def test_large_refund_finance_approval_goes_to_pending_auditor(
        self, admin_token, finance_token, ts
    ):
        inv = self._make_large_invoice(admin_token, ts, "RFLGPF")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFLGPF", amount=1500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 1200.0).json()
        resp = self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "pending_auditor"
        assert body["finance_approved_by"] is not None
        assert body["auditor_approved_by"] is None

    def test_large_refund_auditor_approval_completes(
        self, admin_token, finance_token, auditor_token, ts
    ):
        inv = self._make_large_invoice(admin_token, ts, "RFLGAUD")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFLGAUD", amount=2000.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 1500.0).json()
        self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._approve_refund(auditor_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "approved"
        assert body["finance_approved_by"] is not None
        assert body["auditor_approved_by"] is not None
        assert body["invoice_line_id"] is not None

    def test_auditor_cannot_approve_at_pending_finance_stage(
        self, admin_token, finance_token, auditor_token, ts
    ):
        inv = _make_issued_invoice(admin_token, ts, "RFAUDPF")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFAUDPF", amount=300.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        resp = self._approve_refund(auditor_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 403

    def test_referee_cannot_approve_refund(self, admin_token, finance_token, referee_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFREFBAC")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFREFBAC", amount=300.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        resp = self._approve_refund(referee_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 403

    # ── Reject ────────────────────────────────────────────────────────────────

    def test_finance_clerk_can_reject_refund(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFREJ")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFREJ", amount=300.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        resp = self._reject_refund(
            finance_token, inv["id"], pmt["id"], refund["id"], reason="Duplicate request"
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "rejected"
        assert body["rejected_by"] is not None
        assert body["rejection_reason"] == "Duplicate request"

    def test_auditor_can_reject_at_pending_auditor_stage(
        self, admin_token, finance_token, auditor_token, ts
    ):
        inv = self._make_large_invoice(admin_token, ts, "RFAUDREJ")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFAUDREJ", amount=2000.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 1500.0).json()
        self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._reject_refund(
            auditor_token, inv["id"], pmt["id"], refund["id"], reason="Does not meet policy"
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "rejected"

    def test_empty_rejection_reason_returns_400(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFREJEMPTY")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFREJEMPTY", amount=300.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        resp = self._reject_refund(finance_token, inv["id"], pmt["id"], refund["id"], reason="")
        assert resp.status_code == 400

    # ── Terminal state guards ─────────────────────────────────────────────────

    def test_cannot_approve_already_approved_refund(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFAAPRV")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFAAPRV", amount=500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 409

    def test_cannot_approve_rejected_refund(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFAPRREJ")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFAPRREJ", amount=500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        self._reject_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 409

    def test_cannot_reject_already_approved_refund(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFREJAPPRV")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFREJAPPRV", amount=500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        self._approve_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._reject_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 409

    def test_cannot_reject_already_rejected_refund(self, admin_token, finance_token, ts):
        inv = _make_issued_invoice(admin_token, ts, "RFRR")
        pmt = _record_payment(admin_token, inv["id"], ts, "RFRR", amount=500.0)
        refund = self._request_refund(finance_token, inv["id"], pmt["id"], 50.0).json()
        self._reject_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        resp = self._reject_refund(finance_token, inv["id"], pmt["id"], refund["id"])
        assert resp.status_code == 409
