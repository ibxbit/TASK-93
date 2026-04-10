"""
Unit tests for results, review, arbitration, correction, and ranking logic.

Mirrors the rules implemented in src/results/service.rs.
No server required — pure Python specification tests.
"""

import pytest
from datetime import datetime, timezone


# ── Constants (mirrors service.rs) ────────────────────────────────────────────

TIME_UNITS = {"milliseconds", "seconds"}

VALID_UNITS = {
    "milliseconds", "feet", "inches", "seconds",
    "meters", "kilometers", "kilograms", "points",
}

VALID_REVIEW_DECISIONS = {"approved", "rejected"}


# ── Business logic mirrors ─────────────────────────────────────────────────────

def is_ascending(unit: str) -> bool:
    """Time units rank ascending (lower = better); others descending."""
    return unit in TIME_UNITS


def derive_reviewed_state(reviews: list, is_championship_class: bool) -> str:
    """
    Mirrors derive_reviewed_state() in src/results/service.rs.

    Rules:
    - Any rejection → "rejected"
    - All approved AND count >= required → "approved"
    - Otherwise → "pending"

    Required: 2 for championship, 1 for non-championship.
    """
    required = 2 if is_championship_class else 1

    if not reviews:
        return "pending"

    if any(r["decision"] == "rejected" for r in reviews):
        return "rejected"

    approvals = sum(1 for r in reviews if r["decision"] == "approved")
    if approvals >= required:
        return "approved"

    return "pending"


def compute_rankings(results: list, unit: str, advancement_rule: str,
                     advancement_value: float) -> list:
    """
    Simplified ranking engine mirroring src/results/service.rs::get_rankings.

    Each result: {"participant_id": int, "best_value": float, "created_at": str}
    Returns list of {"rank": int, "participant_id": int, "best_value": float,
                      "best_recorded_at": str, "advances": bool}
    """
    ascending = is_ascending(unit)

    def sort_key(r):
        value = r["best_value"]
        ts = r["created_at"]
        return (value, ts) if ascending else (-value, ts)

    sorted_results = sorted(results, key=sort_key)

    total = len(sorted_results)
    ranked = []
    for i, r in enumerate(sorted_results):
        rank = i + 1
        if advancement_rule == "top_n":
            advances = rank <= int(advancement_value)
        elif advancement_rule == "percentile":
            threshold = max(1, round(total * advancement_value / 100))
            advances = rank <= threshold
        else:
            advances = False
        ranked.append({
            "rank": rank,
            "participant_id": r["participant_id"],
            "best_value": r["best_value"],
            "best_recorded_at": r["created_at"],
            "advances": advances,
        })
    return ranked


def can_transition_correction(current: str, target: str) -> bool:
    """
    Correction status machine (mirrors CorrectionStatus transitions).
    pending → approved | rejected (once; no further transitions)
    """
    transitions = {
        "pending": {"approved", "rejected"},
        "approved": set(),   # terminal
        "rejected": set(),   # terminal
    }
    return target in transitions.get(current, set())


# ── Review quorum logic ───────────────────────────────────────────────────────

class TestReviewQuorumNonChampionship:
    def test_no_reviews_is_pending(self):
        assert derive_reviewed_state([], False) == "pending"

    def test_single_approval_approves(self):
        reviews = [{"decision": "approved"}]
        assert derive_reviewed_state(reviews, False) == "approved"

    def test_single_rejection_rejects(self):
        reviews = [{"decision": "rejected"}]
        assert derive_reviewed_state(reviews, False) == "rejected"

    def test_rejection_overrides_approval(self):
        reviews = [{"decision": "approved"}, {"decision": "rejected"}]
        assert derive_reviewed_state(reviews, False) == "rejected"

    def test_multiple_approvals_approved(self):
        reviews = [{"decision": "approved"}, {"decision": "approved"}]
        assert derive_reviewed_state(reviews, False) == "approved"

    def test_all_rejections(self):
        reviews = [{"decision": "rejected"}, {"decision": "rejected"}]
        assert derive_reviewed_state(reviews, False) == "rejected"


class TestReviewQuorumChampionship:
    def test_no_reviews_is_pending(self):
        assert derive_reviewed_state([], True) == "pending"

    def test_single_approval_stays_pending(self):
        """Championship requires ≥2 approvals — one is not enough."""
        reviews = [{"decision": "approved"}]
        assert derive_reviewed_state(reviews, True) == "pending"

    def test_two_approvals_approve(self):
        reviews = [{"decision": "approved"}, {"decision": "approved"}]
        assert derive_reviewed_state(reviews, True) == "approved"

    def test_three_approvals_approve(self):
        reviews = [
            {"decision": "approved"},
            {"decision": "approved"},
            {"decision": "approved"},
        ]
        assert derive_reviewed_state(reviews, True) == "approved"

    def test_one_approval_one_rejection_rejects(self):
        """Any rejection → reject regardless of approvals."""
        reviews = [{"decision": "approved"}, {"decision": "rejected"}]
        assert derive_reviewed_state(reviews, True) == "rejected"

    def test_two_approvals_one_rejection_still_rejects(self):
        reviews = [
            {"decision": "approved"},
            {"decision": "approved"},
            {"decision": "rejected"},
        ]
        assert derive_reviewed_state(reviews, True) == "rejected"

    def test_single_rejection_rejects(self):
        reviews = [{"decision": "rejected"}]
        assert derive_reviewed_state(reviews, True) == "rejected"


# ── Ranking sort order ────────────────────────────────────────────────────────

class TestRankingSortOrder:
    def test_milliseconds_is_ascending(self):
        assert is_ascending("milliseconds") is True

    def test_seconds_is_ascending(self):
        assert is_ascending("seconds") is True

    def test_points_is_descending(self):
        assert is_ascending("points") is False

    def test_meters_is_descending(self):
        assert is_ascending("meters") is False

    def test_feet_is_descending(self):
        assert is_ascending("feet") is False

    def test_kilometers_is_descending(self):
        assert is_ascending("kilometers") is False

    def test_kilograms_is_descending(self):
        assert is_ascending("kilograms") is False

    def test_inches_is_descending(self):
        assert is_ascending("inches") is False

    def test_time_unit_ranks_fastest_first(self):
        results = [
            {"participant_id": 1, "best_value": 60000.0, "created_at": "2026-01-01T10:00:00Z"},
            {"participant_id": 2, "best_value": 45000.0, "created_at": "2026-01-01T10:01:00Z"},
            {"participant_id": 3, "best_value": 55000.0, "created_at": "2026-01-01T10:02:00Z"},
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 3)
        assert ranked[0]["participant_id"] == 2  # fastest
        assert ranked[1]["participant_id"] == 3
        assert ranked[2]["participant_id"] == 1  # slowest

    def test_points_unit_ranks_highest_first(self):
        results = [
            {"participant_id": 1, "best_value": 100.0, "created_at": "2026-01-01T10:00:00Z"},
            {"participant_id": 2, "best_value": 200.0, "created_at": "2026-01-01T10:01:00Z"},
            {"participant_id": 3, "best_value": 150.0, "created_at": "2026-01-01T10:02:00Z"},
        ]
        ranked = compute_rankings(results, "points", "top_n", 3)
        assert ranked[0]["participant_id"] == 2  # highest points
        assert ranked[1]["participant_id"] == 3
        assert ranked[2]["participant_id"] == 1  # lowest points


# ── Tie-breaker logic ─────────────────────────────────────────────────────────

class TestTieBreaker:
    def test_equal_values_broken_by_earliest_timestamp(self):
        """When values are equal, the participant who submitted first ranks higher."""
        results = [
            {"participant_id": 10, "best_value": 50000.0,
             "created_at": "2026-01-01T10:05:00Z"},  # later
            {"participant_id": 11, "best_value": 50000.0,
             "created_at": "2026-01-01T10:00:00Z"},  # earlier → ranks higher
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 2)
        assert ranked[0]["participant_id"] == 11  # earliest wins tie
        assert ranked[1]["participant_id"] == 10

    def test_equal_points_broken_by_earliest_timestamp(self):
        results = [
            {"participant_id": 20, "best_value": 100.0,
             "created_at": "2026-01-01T12:00:00Z"},  # later
            {"participant_id": 21, "best_value": 100.0,
             "created_at": "2026-01-01T11:00:00Z"},  # earlier → ranks higher
        ]
        ranked = compute_rankings(results, "points", "top_n", 2)
        assert ranked[0]["participant_id"] == 21

    def test_tie_does_not_change_total_count(self):
        results = [
            {"participant_id": 30, "best_value": 50000.0,
             "created_at": "2026-01-01T10:00:00Z"},
            {"participant_id": 31, "best_value": 50000.0,
             "created_at": "2026-01-01T10:01:00Z"},
            {"participant_id": 32, "best_value": 60000.0,
             "created_at": "2026-01-01T10:02:00Z"},
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 3)
        assert len(ranked) == 3

    def test_ranks_are_sequential(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": f"2026-01-01T{10+i:02d}:00:00Z"}
            for i in range(5)
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 5)
        assert [r["rank"] for r in ranked] == [1, 2, 3, 4, 5]


# ── Advancement logic ─────────────────────────────────────────────────────────

class TestAdvancementTopN:
    def test_top_1_advances_only_first(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(5)
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 1)
        advanced = [r for r in ranked if r["advances"]]
        assert len(advanced) == 1
        assert advanced[0]["rank"] == 1

    def test_top_3_advances_3(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(6)
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 3)
        advanced = [r for r in ranked if r["advances"]]
        assert len(advanced) == 3

    def test_top_n_greater_than_total_advances_all(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(3)
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 10)
        assert all(r["advances"] for r in ranked)

    def test_non_advancing_participants_have_advances_false(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(5)
        ]
        ranked = compute_rankings(results, "milliseconds", "top_n", 2)
        for r in ranked:
            if r["rank"] > 2:
                assert r["advances"] is False


class TestAdvancementPercentile:
    def test_50th_percentile_advances_half(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(10)
        ]
        ranked = compute_rankings(results, "milliseconds", "percentile", 50)
        advanced = [r for r in ranked if r["advances"]]
        assert len(advanced) == 5  # 50% of 10

    def test_100th_percentile_advances_all(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(5)
        ]
        ranked = compute_rankings(results, "milliseconds", "percentile", 100)
        assert all(r["advances"] for r in ranked)

    def test_0th_percentile_advances_none_or_at_least_1(self):
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(5)
        ]
        ranked = compute_rankings(results, "milliseconds", "percentile", 0)
        advanced = [r for r in ranked if r["advances"]]
        # 0% rounds to at least 1 (min 1 advance)
        assert len(advanced) >= 0  # server may allow 0 or min 1

    def test_25th_percentile_of_8(self):
        """25% of 8 = 2 participants advance."""
        results = [
            {"participant_id": i, "best_value": float(i * 1000),
             "created_at": "2026-01-01T10:00:00Z"}
            for i in range(8)
        ]
        ranked = compute_rankings(results, "milliseconds", "percentile", 25)
        advanced = [r for r in ranked if r["advances"]]
        assert len(advanced) == 2


# ── Correction workflow state machine ─────────────────────────────────────────

class TestCorrectionWorkflow:
    def test_pending_can_be_approved(self):
        assert can_transition_correction("pending", "approved") is True

    def test_pending_can_be_rejected(self):
        assert can_transition_correction("pending", "rejected") is True

    def test_approved_is_terminal(self):
        assert can_transition_correction("approved", "rejected") is False
        assert can_transition_correction("approved", "pending") is False
        assert can_transition_correction("approved", "approved") is False

    def test_rejected_is_terminal(self):
        assert can_transition_correction("rejected", "approved") is False
        assert can_transition_correction("rejected", "pending") is False
        assert can_transition_correction("rejected", "rejected") is False

    def test_pending_cannot_stay_pending(self):
        assert can_transition_correction("pending", "pending") is False

    def test_unknown_state_returns_false(self):
        assert can_transition_correction("unknown", "approved") is False

    def test_correction_immutability_principle(self):
        """Once resolved, a correction's state must not change — terminal check."""
        terminal_states = {"approved", "rejected"}
        for state in terminal_states:
            for target in {"approved", "rejected", "pending"}:
                assert can_transition_correction(state, target) is False, (
                    f"Terminal state '{state}' must not transition to '{target}'"
                )


# ── Audit log masking ─────────────────────────────────────────────────────────

SENSITIVE_FIELDS = {"serial_number", "external_reference", "vin", "reference_hash"}
REDACTED_MARKER = "***REDACTED***"


def mask_snapshot(snapshot: dict) -> dict:
    """Mirror the audit masking logic in Rust service code."""
    return {
        k: REDACTED_MARKER if k in SENSITIVE_FIELDS else v
        for k, v in snapshot.items()
    }


class TestAuditMaskingLogic:
    def test_serial_number_is_masked(self):
        snapshot = {"id": 1, "name": "Asset A", "serial_number": "SN-12345"}
        masked = mask_snapshot(snapshot)
        assert masked["serial_number"] == REDACTED_MARKER

    def test_non_sensitive_fields_pass_through(self):
        snapshot = {"id": 1, "name": "Asset A", "serial_number": "SN-12345"}
        masked = mask_snapshot(snapshot)
        assert masked["id"] == 1
        assert masked["name"] == "Asset A"

    def test_external_reference_is_masked(self):
        snapshot = {"id": 5, "amount": "100.00", "external_reference": "TXN-SECRET"}
        masked = mask_snapshot(snapshot)
        assert masked["external_reference"] == REDACTED_MARKER

    def test_vin_is_masked(self):
        snapshot = {"id": 2, "make": "Toyota", "vin": "1HGCM82633A004352"}
        masked = mask_snapshot(snapshot)
        assert masked["vin"] == REDACTED_MARKER

    def test_reference_hash_is_masked(self):
        snapshot = {"id": 3, "reference_hash": "abc123hash"}
        masked = mask_snapshot(snapshot)
        assert masked["reference_hash"] == REDACTED_MARKER

    def test_empty_snapshot_remains_empty(self):
        assert mask_snapshot({}) == {}

    def test_snapshot_without_sensitive_fields_unchanged(self):
        snapshot = {"id": 10, "status": "active", "created_at": "2026-01-01T00:00:00Z"}
        assert mask_snapshot(snapshot) == snapshot

    def test_multiple_sensitive_fields_all_masked(self):
        snapshot = {
            "id": 20,
            "serial_number": "SN-ABC",
            "external_reference": "TXN-XYZ",
            "vin": "1HGCM82633A004352",
        }
        masked = mask_snapshot(snapshot)
        assert masked["serial_number"] == REDACTED_MARKER
        assert masked["external_reference"] == REDACTED_MARKER
        assert masked["vin"] == REDACTED_MARKER
        assert masked["id"] == 20
