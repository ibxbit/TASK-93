"""
Unit tests for input validation rules and HTTP error contract.

Tests verify the expected API contract for valid/invalid inputs and
the standard error response shape.  No server required.
"""

import pytest
import json
import re


# ── Error response schema ─────────────────────────────────────────────────────

EXPECTED_ERROR_CODES = {
    400: "BAD_REQUEST",
    401: "UNAUTHORIZED",
    403: "FORBIDDEN",
    404: "NOT_FOUND",
    409: "CONFLICT",
    422: "UNPROCESSABLE_ENTITY",
    500: "INTERNAL_ERROR",
}


def validate_error_body(body: dict, expected_status: int) -> bool:
    """Validate the standard error response shape."""
    required_keys = {"code", "message"}
    if not required_keys.issubset(body.keys()):
        return False
    expected_code = EXPECTED_ERROR_CODES.get(expected_status)
    return body["code"] == expected_code and isinstance(body["message"], str)


class TestErrorResponseSchema:
    def test_not_found_error_shape(self):
        body = {"code": "NOT_FOUND", "message": "Resource not found: vehicle 999"}
        assert validate_error_body(body, 404) is True

    def test_bad_request_error_shape(self):
        body = {"code": "BAD_REQUEST", "message": "Bad request: invalid field"}
        assert validate_error_body(body, 400) is True

    def test_unauthorized_error_shape(self):
        body = {"code": "UNAUTHORIZED", "message": "Unauthorized: Invalid credentials"}
        assert validate_error_body(body, 401) is True

    def test_forbidden_error_shape(self):
        body = {"code": "FORBIDDEN", "message": "Forbidden: Insufficient permissions"}
        assert validate_error_body(body, 403) is True

    def test_conflict_error_shape(self):
        body = {"code": "CONFLICT", "message": "Conflict: duplicate key"}
        assert validate_error_body(body, 409) is True

    def test_unprocessable_error_shape(self):
        body = {"code": "UNPROCESSABLE_ENTITY", "message": "Unprocessable entity: ..."}
        assert validate_error_body(body, 422) is True

    def test_wrong_code_fails_validation(self):
        body = {"code": "WRONG_CODE", "message": "some error"}
        assert validate_error_body(body, 404) is False

    def test_missing_message_fails_validation(self):
        body = {"code": "NOT_FOUND"}
        assert validate_error_body(body, 404) is False

    def test_correlation_id_is_optional(self):
        # correlation_id is omitted when not available — body is still valid
        body = {"code": "NOT_FOUND", "message": "Not found"}
        assert validate_error_body(body, 404) is True
        body_with_cid = {"code": "NOT_FOUND", "message": "Not found", "correlation_id": "abc-123"}
        assert validate_error_body(body_with_cid, 404) is True


# ── Payment lifecycle and exception states ─────────────────────────────────────

EXCEPTION_TYPES = {"void", "reversal", "dispute"}
PAYMENT_ENTRY_STATUSES = {"active", "voided", "reversed", "disputed"}

PAYMENT_STATUS_FLOW = {
    "active": {"voided", "reversed", "disputed"},
    "voided": set(),
    "reversed": set(),
    "disputed": set(),
}


def can_transition_payment(current: str, target: str) -> bool:
    return target in PAYMENT_STATUS_FLOW.get(current, set())


REFUND_STATUS_FLOW = {
    "pending_finance": {"pending_auditor", "approved", "rejected"},
    "pending_auditor": {"approved", "rejected"},
    "approved": set(),
    "rejected": set(),
}


def can_transition_refund(current: str, target: str) -> bool:
    return target in REFUND_STATUS_FLOW.get(current, set())


class TestPaymentLifecycle:
    def test_active_can_be_voided(self):
        assert can_transition_payment("active", "voided") is True

    def test_active_can_be_reversed(self):
        assert can_transition_payment("active", "reversed") is True

    def test_active_can_be_disputed(self):
        assert can_transition_payment("active", "disputed") is True

    def test_voided_is_terminal(self):
        assert can_transition_payment("voided", "active") is False

    def test_exception_types_valid(self):
        for e in ["void", "reversal", "dispute"]:
            assert e in EXCEPTION_TYPES


class TestRefundWorkflow:
    def test_pending_finance_to_approved_for_small_amounts(self):
        assert can_transition_refund("pending_finance", "approved") is True

    def test_pending_finance_to_auditor_for_large_amounts(self):
        assert can_transition_refund("pending_finance", "pending_auditor") is True

    def test_pending_auditor_to_approved(self):
        assert can_transition_refund("pending_auditor", "approved") is True

    def test_terminal_states(self):
        for terminal in ["approved", "rejected"]:
            assert can_transition_refund(terminal, "pending_finance") is False


# ── Pricing model validation ───────────────────────────────────────────────────

VALID_PRICING_MODELS = {"per_unit", "per_duration", "package", "fixed", "percentage"}


def is_valid_pricing_model(model: str) -> bool:
    return model in VALID_PRICING_MODELS


class TestPricingModelValidation:
    def test_per_unit_valid(self):
        assert is_valid_pricing_model("per_unit") is True

    def test_per_duration_valid(self):
        assert is_valid_pricing_model("per_duration") is True

    def test_package_valid(self):
        assert is_valid_pricing_model("package") is True

    def test_fixed_valid(self):
        assert is_valid_pricing_model("fixed") is True

    def test_percentage_valid(self):
        assert is_valid_pricing_model("percentage") is True

    def test_unknown_model_rejected(self):
        assert is_valid_pricing_model("hourly") is False
        assert is_valid_pricing_model("") is False
        assert is_valid_pricing_model("FIXED") is False


# ── Invoice number format ─────────────────────────────────────────────────────

def is_valid_invoice_no(invoice_no: str) -> bool:
    """Invoice number must be non-empty and not exceed 100 chars."""
    return bool(invoice_no) and len(invoice_no) <= 100


class TestInvoiceNumberValidation:
    def test_standard_format_valid(self):
        assert is_valid_invoice_no("INV-2026-0042") is True

    def test_simple_format_valid(self):
        assert is_valid_invoice_no("001") is True

    def test_empty_rejected(self):
        assert is_valid_invoice_no("") is False

    def test_too_long_rejected(self):
        assert is_valid_invoice_no("X" * 101) is False

    def test_max_length_accepted(self):
        assert is_valid_invoice_no("X" * 100) is True


# ── Tax rate validation ────────────────────────────────────────────────────────

def is_valid_tax_rate(rate: float) -> bool:
    """Tax rate must be in [0.0, 1.0]."""
    return 0.0 <= rate <= 1.0


class TestTaxRateValidation:
    def test_zero_tax_valid(self):
        assert is_valid_tax_rate(0.0) is True

    def test_ten_percent_valid(self):
        assert is_valid_tax_rate(0.10) is True

    def test_hundred_percent_valid(self):
        assert is_valid_tax_rate(1.0) is True

    def test_negative_tax_rejected(self):
        assert is_valid_tax_rate(-0.01) is False

    def test_over_one_rejected(self):
        assert is_valid_tax_rate(1.01) is False

    def test_ten_as_percent_rejected(self):
        # API expects fraction, not percentage; 10 (meaning 10%) is out of range
        assert is_valid_tax_rate(10.0) is False


# ── Quantity and price validation ─────────────────────────────────────────────

def is_valid_quantity(qty: float) -> bool:
    return qty > 0.0


def is_valid_unit_price(price: float) -> bool:
    return price >= 0.0


class TestLineItemValidation:
    def test_positive_quantity_valid(self):
        assert is_valid_quantity(1.0) is True
        assert is_valid_quantity(0.5) is True
        assert is_valid_quantity(100.0) is True

    def test_zero_quantity_rejected(self):
        assert is_valid_quantity(0.0) is False

    def test_negative_quantity_rejected(self):
        assert is_valid_quantity(-1.0) is False

    def test_zero_unit_price_valid(self):
        # Free line items are allowed
        assert is_valid_unit_price(0.0) is True

    def test_positive_unit_price_valid(self):
        assert is_valid_unit_price(99.99) is True

    def test_negative_unit_price_rejected(self):
        assert is_valid_unit_price(-1.0) is False


# ── Payment method validation ─────────────────────────────────────────────────

VALID_PAYMENT_METHODS = {"cash", "cheque", "check", "ach", "bank_transfer", "card"}


def is_valid_payment_method(method: str) -> bool:
    return method in VALID_PAYMENT_METHODS


class TestPaymentMethodValidation:
    def test_cash_valid(self):
        assert is_valid_payment_method("cash") is True

    def test_cheque_valid(self):
        assert is_valid_payment_method("cheque") is True

    def test_check_alias_valid(self):
        assert is_valid_payment_method("check") is True

    def test_ach_valid(self):
        assert is_valid_payment_method("ach") is True

    def test_bank_transfer_valid(self):
        assert is_valid_payment_method("bank_transfer") is True

    def test_card_valid(self):
        assert is_valid_payment_method("card") is True

    def test_credit_card_rejected(self):
        assert is_valid_payment_method("credit_card") is False

    def test_unknown_method_rejected(self):
        assert is_valid_payment_method("crypto") is False
        assert is_valid_payment_method("") is False


# ── Payment lifecycle and exception states ─────────────────────────────────────

EXCEPTION_TYPES = {"void", "reversal", "dispute"}
PAYMENT_ENTRY_STATUSES = {"active", "voided", "reversed", "disputed"}

PAYMENT_STATUS_FLOW = {
    "active": {"voided", "reversed", "disputed"},
    "voided": set(),
    "reversed": set(),
    "disputed": set(),
}


def can_transition_payment(current: str, target: str) -> bool:
    return target in PAYMENT_STATUS_FLOW.get(current, set())


REFUND_STATUS_FLOW = {
    "pending_finance": {"pending_auditor", "approved", "rejected"},
    "pending_auditor": {"approved", "rejected"},
    "approved": set(),
    "rejected": set(),
}


def can_transition_refund(current: str, target: str) -> bool:
    return target in REFUND_STATUS_FLOW.get(current, set())


class TestPaymentLifecycle:
    def test_active_can_be_voided(self):
        assert can_transition_payment("active", "voided") is True

    def test_active_can_be_reversed(self):
        assert can_transition_payment("active", "reversed") is True

    def test_active_can_be_disputed(self):
        assert can_transition_payment("active", "disputed") is True

    def test_voided_is_terminal(self):
        assert can_transition_payment("voided", "active") is False

    def test_exception_types_valid(self):
        for e in ["void", "reversal", "dispute"]:
            assert e in EXCEPTION_TYPES


class TestRefundWorkflow:
    def test_pending_finance_to_approved_for_small_amounts(self):
        assert can_transition_refund("pending_finance", "approved") is True

    def test_pending_finance_to_auditor_for_large_amounts(self):
        assert can_transition_refund("pending_finance", "pending_auditor") is True

    def test_pending_auditor_to_approved(self):
        assert can_transition_refund("pending_auditor", "approved") is True

    def test_terminal_states(self):
        for terminal in ["approved", "rejected"]:
            assert can_transition_refund(terminal, "pending_finance") is False


# ── Event status transitions ───────────────────────────────────────────────────

EVENT_STATUS_FLOW = {
    "draft": {"published"},
    "published": {"in_progress", "cancelled"},
    "in_progress": {"completed", "cancelled"},
    "completed": set(),
    "cancelled": set(),
}


def can_transition_event(current: str, target: str) -> bool:
    return target in EVENT_STATUS_FLOW.get(current, set())


class TestEventStatusTransitions:
    def test_draft_to_published_via_publish_endpoint(self):
        # The publish endpoint transitions draft → published
        assert can_transition_event("draft", "published") is True

    def test_draft_cannot_skip_to_in_progress(self):
        assert can_transition_event("draft", "in_progress") is False

    def test_completed_is_terminal(self):
        assert can_transition_event("completed", "draft") is False

    def test_cancelled_is_terminal(self):
        assert can_transition_event("cancelled", "in_progress") is False


# ── Audit log query limits ─────────────────────────────────────────────────────

MAX_AUDIT_LIMIT = 500
DEFAULT_AUDIT_LIMIT = 100


def cap_audit_limit(requested: int) -> int:
    """Audit list endpoint caps results at 500."""
    return min(max(requested, 1), MAX_AUDIT_LIMIT)


class TestAuditLogQueryLimits:
    def test_limit_capped_at_500(self):
        assert cap_audit_limit(1000) == MAX_AUDIT_LIMIT

    def test_limit_100_passes_through(self):
        assert cap_audit_limit(100) == 100

    def test_limit_500_passes_through(self):
        assert cap_audit_limit(500) == MAX_AUDIT_LIMIT

    def test_default_is_100(self):
        assert DEFAULT_AUDIT_LIMIT == 100

    def test_zero_normalized_to_one(self):
        assert cap_audit_limit(0) == 1
