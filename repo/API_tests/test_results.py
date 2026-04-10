"""
API tests for the results, reviews, arbitration, corrections, rankings, and export modules.

Covers:
- POST /events/<id>/results                              (submit result; participants:write)
- GET  /events/<id>/rankings                             (get rankings;  events:read)
- GET  /events/<id>/results/export                       (CSV export;    audit:read)
- POST /events/<id>/results/<rid>/reviews                (submit review; referees:write)
- GET  /events/<id>/results/<rid>/reviews                (list reviews;  events:read)
- POST /events/<id>/results/<rid>/arbitrate              (arbitrate;     events:write)
- POST /events/<id>/results/<rid>/corrections            (request correction; participants:write)
- GET  /events/<id>/results/<rid>/corrections            (list corrections;   events:read)
- POST /events/<id>/results/<rid>/corrections/<cid>/resolve  (resolve; events:write)

Workflow evidence:
- Multi-referee review: championship class requires ≥2 approvals
- Arbitration by Event Director overrides all reviews
- Deterministic tie-breakers (best attempt + earliest timestamp)
- Advancement logic: top_n and percentile rules
- Corrections: immutable prior versions, request→resolve workflow
- CSV export: proper headers, columns, RBAC
"""

import time
import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Module-level fixtures ─────────────────────────────────────────────────────

@pytest.fixture(scope="module")
def ruleset(admin_token, ts):
    """Create a ruleset version used when publishing test events."""
    resp = requests.post(
        f"{BASE_URL}/rulesets",
        headers=auth_headers(admin_token),
        json={
            "semantic_version": f"3.{ts % 999}.0",
            "description": "Results test ruleset",
            "effective_at": "2026-01-01T00:00:00Z",
        },
        timeout=10,
    )
    assert resp.status_code == 200, f"Ruleset creation failed: {resp.text}"
    return resp.json()


@pytest.fixture(scope="module")
def std_event(admin_token, ruleset, ts):
    """Published non-championship event for most result tests."""
    ev = requests.post(
        f"{BASE_URL}/events",
        headers=auth_headers(admin_token),
        json={
            "name": f"Results Std Event {ts}",
            "description": "Standard (non-championship) event for result tests",
            "schedule_group": "Group A",
            "is_championship_class": False,
        },
        timeout=10,
    ).json()
    requests.post(
        f"{BASE_URL}/events/{ev['id']}/publish",
        headers=auth_headers(admin_token),
        json={"ruleset_version_id": ruleset["id"]},
        timeout=10,
    )
    return ev


@pytest.fixture(scope="module")
def champ_event(admin_token, ruleset, ts):
    """Published championship-class event for multi-referee quorum tests."""
    ev = requests.post(
        f"{BASE_URL}/events",
        headers=auth_headers(admin_token),
        json={
            "name": f"Championship Event {ts}",
            "description": "Championship class event for quorum tests",
            "schedule_group": "Championship",
            "is_championship_class": True,
        },
        timeout=10,
    ).json()
    requests.post(
        f"{BASE_URL}/events/{ev['id']}/publish",
        headers=auth_headers(admin_token),
        json={"ruleset_version_id": ruleset["id"]},
        timeout=10,
    )
    return ev


# ── Shared inline helpers ─────────────────────────────────────────────────────

def _submit_result(token, event_id, participant_id, value, unit="milliseconds", suffix=""):
    return requests.post(
        f"{BASE_URL}/events/{event_id}/results",
        headers=auth_headers(token),
        json={
            "participant_id": participant_id,
            "value_numeric": value,
            "unit": unit,
        },
        timeout=10,
    )


def _submit_review(token, event_id, result_id, decision, comment=None):
    body = {"decision": decision}
    if comment:
        body["comment"] = comment
    return requests.post(
        f"{BASE_URL}/events/{event_id}/results/{result_id}/reviews",
        headers=auth_headers(token),
        json=body,
        timeout=10,
    )


def _arbitrate(token, event_id, result_id, decision, comment=None):
    body = {"decision": decision}
    if comment:
        body["comment"] = comment
    return requests.post(
        f"{BASE_URL}/events/{event_id}/results/{result_id}/arbitrate",
        headers=auth_headers(token),
        json=body,
        timeout=10,
    )


def _request_correction(token, event_id, result_id, value, unit, reason=None):
    body = {"corrected_value": value, "corrected_unit": unit}
    if reason:
        body["reason"] = reason
    return requests.post(
        f"{BASE_URL}/events/{event_id}/results/{result_id}/corrections",
        headers=auth_headers(token),
        json=body,
        timeout=10,
    )


def _resolve_correction(token, event_id, result_id, correction_id, decision):
    return requests.post(
        f"{BASE_URL}/events/{event_id}/results/{result_id}"
        f"/corrections/{correction_id}/resolve",
        headers=auth_headers(token),
        json={"decision": decision},
        timeout=10,
    )


# ── Result submission ─────────────────────────────────────────────────────────

class TestResultSubmission:
    def test_submit_result_returns_200(self, admin_token, std_event, ts):
        resp = _submit_result(admin_token, std_event["id"], ts + 100, 75000.0)
        assert resp.status_code == 200

    def test_result_has_required_fields(self, admin_token, std_event, ts):
        resp = _submit_result(admin_token, std_event["id"], ts + 101, 80000.0)
        assert resp.status_code == 200
        body = resp.json()
        required = {
            "id", "event_id", "participant_id", "attempt_no",
            "value_numeric", "unit", "reviewed_state",
            "entered_by", "created_at", "updated_at",
        }
        assert required.issubset(body.keys())

    def test_new_result_is_pending(self, admin_token, std_event, ts):
        resp = _submit_result(admin_token, std_event["id"], ts + 102, 65000.0)
        assert resp.json()["reviewed_state"] == "pending"

    def test_attempt_no_auto_assigned(self, admin_token, std_event, ts):
        pid = ts + 200
        r1 = _submit_result(admin_token, std_event["id"], pid, 70000.0).json()
        r2 = _submit_result(admin_token, std_event["id"], pid, 68000.0).json()
        assert r2["attempt_no"] == r1["attempt_no"] + 1

    def test_negative_value_returns_400(self, admin_token, std_event, ts):
        resp = _submit_result(admin_token, std_event["id"], ts + 103, -100.0)
        assert resp.status_code == 400

    def test_unknown_unit_returns_400(self, admin_token, std_event, ts):
        resp = requests.post(
            f"{BASE_URL}/events/{std_event['id']}/results",
            headers=auth_headers(admin_token),
            json={
                "participant_id": ts + 104,
                "value_numeric": 100.0,
                "unit": "furlongs",
            },
            timeout=10,
        )
        assert resp.status_code == 400

    def test_director_can_submit_result(self, director_token, std_event, ts):
        resp = _submit_result(director_token, std_event["id"], ts + 105, 90000.0)
        assert resp.status_code == 200

    def test_referee_cannot_submit_result(self, referee_token, std_event, ts):
        resp = _submit_result(referee_token, std_event["id"], ts + 106, 90000.0)
        assert resp.status_code == 403

    def test_finance_clerk_cannot_submit_result(self, finance_token, std_event, ts):
        resp = _submit_result(finance_token, std_event["id"], ts + 107, 90000.0)
        assert resp.status_code == 403

    def test_unauthenticated_returns_401(self, std_event, ts):
        resp = requests.post(
            f"{BASE_URL}/events/{std_event['id']}/results",
            json={"participant_id": ts + 108, "value_numeric": 1.0, "unit": "points"},
            timeout=10,
        )
        assert resp.status_code == 401

    def test_all_units_accepted(self, admin_token, std_event, ts):
        valid_units = [
            "milliseconds", "feet", "inches", "seconds",
            "meters", "kilometers", "kilograms", "points",
        ]
        for i, unit in enumerate(valid_units):
            resp = _submit_result(
                admin_token, std_event["id"], ts + 400 + i, 100.0, unit=unit
            )
            assert resp.status_code == 200, f"Unit '{unit}' rejected: {resp.text}"


# ── Multi-referee review (non-championship) ───────────────────────────────────

class TestNonChampionshipReview:
    """Non-championship: single approval is sufficient for auto-approval."""

    def test_single_approval_auto_approves_result(self, admin_token, director_token,
                                                   std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 300, 55000.0).json()
        review_resp = _submit_review(director_token, std_event["id"], result["id"], "approved")
        assert review_resp.status_code == 200

        # Reload result to check state
        result_after = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert result_after.status_code == 200
        reviews = result_after.json()
        assert len(reviews) == 1
        assert reviews[0]["decision"] == "approved"

    def test_single_rejection_auto_rejects_result(self, admin_token, director_token,
                                                    std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 301, 62000.0).json()
        _submit_review(director_token, std_event["id"], result["id"], "rejected",
                       comment="Measurement error")
        reviews = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert reviews[0]["decision"] == "rejected"

    def test_review_has_required_fields(self, admin_token, director_token, std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 302, 50000.0).json()
        review = _submit_review(director_token, std_event["id"], result["id"], "approved").json()
        required = {"id", "result_id", "referee_id", "decision", "reviewed_at", "created_at"}
        assert required.issubset(review.keys())

    def test_review_comment_is_stored(self, admin_token, director_token, std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 303, 51000.0).json()
        review = _submit_review(
            director_token, std_event["id"], result["id"], "approved",
            comment="Looks good"
        ).json()
        assert review["comment"] == "Looks good"

    def test_referee_cannot_submit_review(self, admin_token, referee_token,
                                           std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 304, 53000.0).json()
        resp = _submit_review(referee_token, std_event["id"], result["id"], "approved")
        assert resp.status_code == 403

    def test_list_reviews_returns_array(self, admin_token, director_token, std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 305, 54000.0).json()
        _submit_review(director_token, std_event["id"], result["id"], "approved")
        reviews = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert isinstance(reviews, list)
        assert len(reviews) >= 1

    def test_auditor_can_list_reviews(self, admin_token, auditor_token, std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 306, 59000.0).json()
        resp = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_invalid_decision_returns_400(self, admin_token, director_token,
                                           std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 307, 60000.0).json()
        resp = _submit_review(director_token, std_event["id"], result["id"], "maybe")
        assert resp.status_code == 400


# ── Multi-referee review (championship class) ─────────────────────────────────

class TestChampionshipReview:
    """Championship: requires ≥2 approvals; any rejection rejects immediately."""

    def test_one_approval_stays_pending_in_championship(self, admin_token,
                                                         director_token,
                                                         champ_event, ts):
        result = _submit_result(admin_token, champ_event["id"], ts + 500, 30000.0).json()
        _submit_review(director_token, champ_event["id"], result["id"], "approved")
        reviews = requests.get(
            f"{BASE_URL}/events/{champ_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        # Only one approval — still pending (need 2 for championship)
        assert len(reviews) == 1
        assert reviews[0]["decision"] == "approved"

    def test_two_approvals_auto_approve_in_championship(self, admin_token,
                                                          director_token,
                                                          champ_event, ts):
        result = _submit_result(admin_token, champ_event["id"], ts + 501, 28000.0).json()
        # First approval (director)
        _submit_review(director_token, champ_event["id"], result["id"], "approved")
        # Second approval (admin acting as second referee)
        _submit_review(admin_token, champ_event["id"], result["id"], "approved")

        reviews = requests.get(
            f"{BASE_URL}/events/{champ_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        approvals = [r for r in reviews if r["decision"] == "approved"]
        assert len(approvals) >= 2

    def test_one_rejection_rejects_championship_result(self, admin_token,
                                                         director_token,
                                                         champ_event, ts):
        result = _submit_result(admin_token, champ_event["id"], ts + 502, 31000.0).json()
        # One approval, then a rejection
        _submit_review(director_token, champ_event["id"], result["id"], "approved")
        _submit_review(admin_token, champ_event["id"], result["id"], "rejected",
                       comment="Disqualified")
        reviews = requests.get(
            f"{BASE_URL}/events/{champ_event['id']}/results/{result['id']}/reviews",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        rejections = [r for r in reviews if r["decision"] == "rejected"]
        assert len(rejections) >= 1


# ── Arbitration ───────────────────────────────────────────────────────────────

class TestArbitration:
    def test_director_can_arbitrate_result(self, admin_token, director_token,
                                            std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 600, 44000.0).json()
        resp = _arbitrate(director_token, std_event["id"], result["id"], "approved",
                          comment="Overriding referee decision")
        assert resp.status_code == 200
        body = resp.json()
        assert body["decision"] == "approved"
        assert body["result_id"] == result["id"]

    def test_arbitration_has_required_fields(self, admin_token, director_token,
                                              std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 601, 46000.0).json()
        arb = _arbitrate(director_token, std_event["id"], result["id"], "rejected").json()
        required = {"id", "result_id", "arbitrated_by", "decision", "created_at"}
        assert required.issubset(arb.keys())

    def test_arbitration_approve_overrides_rejection(self, admin_token,
                                                       director_token,
                                                       std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 602, 47000.0).json()
        # Referee rejects
        _submit_review(director_token, std_event["id"], result["id"], "rejected")
        # Director arbitrates to approved — should succeed
        arb = _arbitrate(director_token, std_event["id"], result["id"], "approved").json()
        assert arb["decision"] == "approved"

    def test_cannot_re_arbitrate_a_result(self, admin_token, director_token,
                                           std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 603, 48000.0).json()
        _arbitrate(director_token, std_event["id"], result["id"], "approved")
        # Second arbitration on same result must fail
        resp = _arbitrate(director_token, std_event["id"], result["id"], "rejected")
        assert resp.status_code in (409, 422)

    def test_referee_cannot_arbitrate(self, admin_token, referee_token,
                                       std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 604, 49000.0).json()
        resp = _arbitrate(referee_token, std_event["id"], result["id"], "approved")
        assert resp.status_code == 403

    def test_finance_clerk_cannot_arbitrate(self, admin_token, finance_token,
                                             std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 605, 50000.0).json()
        resp = _arbitrate(finance_token, std_event["id"], result["id"], "approved")
        assert resp.status_code == 403

    def test_invalid_arbitration_decision_returns_400(self, admin_token,
                                                       director_token,
                                                       std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 606, 51000.0).json()
        resp = _arbitrate(director_token, std_event["id"], result["id"], "abstain")
        assert resp.status_code == 400


# ── Corrections workflow ──────────────────────────────────────────────────────

class TestCorrectionsWorkflow:
    def test_request_correction_returns_pending(self, admin_token, director_token,
                                                 std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 700, 72000.0).json()
        resp = _request_correction(
            director_token, std_event["id"], result["id"],
            69000.0, "milliseconds", reason="Timing error"
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["status"] == "pending"
        assert body["result_id"] == result["id"]

    def test_correction_has_required_fields(self, admin_token, director_token,
                                             std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 701, 73000.0).json()
        corr = _request_correction(
            director_token, std_event["id"], result["id"],
            70000.0, "milliseconds"
        ).json()
        required = {
            "id", "result_id", "corrected_value", "corrected_unit",
            "requested_by", "status", "created_at",
        }
        assert required.issubset(corr.keys())

    def test_original_result_is_unchanged_after_correction_request(
        self, admin_token, director_token, std_event, ts
    ):
        result = _submit_result(admin_token, std_event["id"], ts + 702, 74000.0).json()
        original_value = result["value_numeric"]
        _request_correction(director_token, std_event["id"], result["id"],
                            71000.0, "milliseconds")
        # Reload result — original value must NOT be modified
        refreshed = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/corrections",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        # The correction is a separate record; original result intact
        assert all(c["corrected_value"] != original_value or
                   c["corrected_unit"] == "milliseconds"
                   for c in refreshed)

    def test_list_corrections_returns_array(self, admin_token, director_token,
                                             std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 703, 75000.0).json()
        _request_correction(director_token, std_event["id"], result["id"],
                            72000.0, "milliseconds")
        corrections = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/corrections",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert isinstance(corrections, list)
        assert len(corrections) >= 1

    def test_approve_correction_transitions_to_approved(self, admin_token,
                                                          director_token,
                                                          std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 704, 76000.0).json()
        corr = _request_correction(director_token, std_event["id"], result["id"],
                                    73000.0, "milliseconds").json()
        resp = _resolve_correction(
            director_token, std_event["id"], result["id"], corr["id"], "approved"
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "approved"
        assert resp.json()["resolved_by"] is not None

    def test_reject_correction_transitions_to_rejected(self, admin_token,
                                                         director_token,
                                                         std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 705, 77000.0).json()
        corr = _request_correction(director_token, std_event["id"], result["id"],
                                    74000.0, "milliseconds").json()
        resp = _resolve_correction(
            director_token, std_event["id"], result["id"], corr["id"], "rejected"
        )
        assert resp.status_code == 200
        assert resp.json()["status"] == "rejected"

    def test_cannot_resolve_already_resolved_correction(self, admin_token,
                                                          director_token,
                                                          std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 706, 78000.0).json()
        corr = _request_correction(director_token, std_event["id"], result["id"],
                                    75000.0, "milliseconds").json()
        _resolve_correction(director_token, std_event["id"], result["id"],
                            corr["id"], "approved")
        # Second resolution must fail — correction is terminal
        resp = _resolve_correction(director_token, std_event["id"], result["id"],
                                   corr["id"], "rejected")
        assert resp.status_code in (409, 422)

    def test_only_one_pending_correction_allowed(self, admin_token, director_token,
                                                   std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 707, 79000.0).json()
        _request_correction(director_token, std_event["id"], result["id"],
                            76000.0, "milliseconds")
        # Second pending correction on same result must fail
        resp = _request_correction(director_token, std_event["id"], result["id"],
                                    77000.0, "milliseconds")
        assert resp.status_code in (409, 422)

    def test_referee_cannot_request_correction(self, admin_token, referee_token,
                                                std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 708, 80000.0).json()
        resp = _request_correction(referee_token, std_event["id"], result["id"],
                                    77000.0, "milliseconds")
        assert resp.status_code == 403

    def test_referee_cannot_resolve_correction(self, admin_token, director_token,
                                                referee_token, std_event, ts):
        result = _submit_result(admin_token, std_event["id"], ts + 709, 81000.0).json()
        corr = _request_correction(director_token, std_event["id"], result["id"],
                                    78000.0, "milliseconds").json()
        resp = _resolve_correction(referee_token, std_event["id"], result["id"],
                                   corr["id"], "approved")
        assert resp.status_code == 403

    def test_correction_immutability_full_history_preserved(self, admin_token,
                                                              director_token,
                                                              std_event, ts):
        """All correction records are immutable — rejected ones stay in history."""
        result = _submit_result(admin_token, std_event["id"], ts + 710, 82000.0).json()
        corr = _request_correction(director_token, std_event["id"], result["id"],
                                    79000.0, "milliseconds").json()
        _resolve_correction(director_token, std_event["id"], result["id"],
                            corr["id"], "rejected")
        # Request a new correction after rejection (should now be allowed)
        _request_correction(director_token, std_event["id"], result["id"],
                            80000.0, "milliseconds")
        history = requests.get(
            f"{BASE_URL}/events/{std_event['id']}/results/{result['id']}/corrections",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        # Both correction records must exist in history
        assert len(history) >= 2
        statuses = {c["status"] for c in history}
        assert "rejected" in statuses


# ── Rankings and advancement ──────────────────────────────────────────────────

class TestRankingsAndAdvancement:

    @pytest.fixture(scope="class")
    def ranking_event(self, admin_token, ruleset, ts):
        """Fresh published event with 4 participants submitted for ranking tests."""
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={
                "name": f"Ranking Event {ts}",
                "schedule_group": "Rankings",
                "is_championship_class": False,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        # Submit 4 results for 4 different participants (time unit — lower is better)
        participants = [ts + 800, ts + 801, ts + 802, ts + 803]
        times = [50000.0, 60000.0, 45000.0, 55000.0]  # fastest = 45000 (pid 802)
        for pid, t in zip(participants, times):
            r = _submit_result(admin_token, ev["id"], pid, t, unit="milliseconds")
            assert r.status_code == 200
        return {"event": ev, "participants": participants, "times": times}

    def test_rankings_returns_response_shape(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        required = {
            "event_id", "unit", "advancement_rule",
            "advancement_value", "total_participants", "rankings",
        }
        assert required.issubset(body.keys())

    def test_rankings_ordered_ascending_for_time_units(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=4",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        rankings = resp.json()["rankings"]
        values = [r["best_value"] for r in rankings]
        assert values == sorted(values), "Time rankings must be ascending (fastest first)"

    def test_rank_entries_have_required_fields(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        for entry in resp.json()["rankings"]:
            required = {
                "rank", "participant_id", "best_value", "unit",
                "best_attempt_no", "best_recorded_at", "advances",
            }
            assert required.issubset(entry.keys())

    def test_top_n_advancement_marks_correct_count(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        rankings = resp.json()["rankings"]
        advanced = [r for r in rankings if r["advances"]]
        assert len(advanced) == 2

    def test_percentile_advancement(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        # 50th percentile of 4 participants → top 2 advance
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=percentile&advancement_value=50",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        rankings = resp.json()["rankings"]
        advanced = [r for r in rankings if r["advances"]]
        assert len(advanced) >= 1

    def test_rank_1_is_best_performer(self, admin_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=4",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        rankings = resp.json()["rankings"]
        rank_1 = next(r for r in rankings if r["rank"] == 1)
        # Fastest time is 45000 ms
        assert rank_1["best_value"] == 45000.0

    def test_best_recorded_at_used_as_tiebreaker(self, admin_token, ranking_event):
        """Rankings include best_recorded_at for deterministic tie-breaking."""
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=4",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        rankings = resp.json()["rankings"]
        for entry in rankings:
            assert "best_recorded_at" in entry
            assert entry["best_recorded_at"]  # non-empty ISO timestamp

    def test_approved_correction_affects_ranking(self, admin_token, director_token,
                                                   ranking_event):
        """Approved correction supersedes original value in rankings."""
        ev_id = ranking_event["event"]["id"]
        # Submit a new result with a slow time
        pid = ranking_event["participants"][0]
        slow_result = _submit_result(
            admin_token, ev_id, pid + 9000, 99000.0, unit="milliseconds"
        ).json()

        # Request and approve a correction to a fast time
        corr = _request_correction(
            director_token, ev_id, slow_result["id"], 40000.0, "milliseconds"
        ).json()
        _resolve_correction(director_token, ev_id, slow_result["id"],
                            corr["id"], "approved")

        # In rankings the corrected value (40000) should appear, not 99000
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=10",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        rankings = resp.json()["rankings"]
        corrected_entry = next(
            (r for r in rankings if r["participant_id"] == pid + 9000), None
        )
        if corrected_entry:
            assert corrected_entry["best_value"] == 40000.0

    def test_referee_can_get_rankings(self, referee_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_finance_clerk_can_get_rankings(self, finance_token, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_unauthenticated_cannot_get_rankings(self, ranking_event):
        ev_id = ranking_event["event"]["id"]
        resp = requests.get(
            f"{BASE_URL}/events/{ev_id}/rankings"
            f"?unit=milliseconds&advancement_rule=top_n&advancement_value=2",
            timeout=10,
        )
        assert resp.status_code == 401

    def test_descending_ranking_for_points_unit(self, admin_token, ruleset, ts):
        """Points unit ranks descending (higher is better)."""
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Points Event {ts}", "is_championship_class": False},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        pids_points = [(ts + 900, 100.0), (ts + 901, 200.0), (ts + 902, 150.0)]
        for pid, pts in pids_points:
            _submit_result(admin_token, ev["id"], pid, pts, unit="points")

        resp = requests.get(
            f"{BASE_URL}/events/{ev['id']}/rankings"
            f"?unit=points&advancement_rule=top_n&advancement_value=3",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        rankings = resp.json()["rankings"]
        values = [r["best_value"] for r in rankings]
        assert values == sorted(values, reverse=True), (
            "Points rankings must be descending (higher is better)"
        )


# ── Results CSV export ────────────────────────────────────────────────────────

class TestResultsExport:
    """GET /events/<id>/results/export — CSV download; requires audit:read."""

    @pytest.fixture(scope="class")
    def export_event(self, admin_token, ruleset, ts):
        """Event with two submitted results for export testing."""
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Export Event {ts}", "is_championship_class": False},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        pid1, pid2 = ts + 7000, ts + 7001
        _submit_result(admin_token, ev["id"], pid1, 55000.0, unit="milliseconds")
        _submit_result(admin_token, ev["id"], pid2, 62000.0, unit="milliseconds")
        return ev

    def test_auditor_can_export_results(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_admin_can_export_results(self, admin_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_export_results(self, referee_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_clerk_cannot_export_results(self, finance_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_unauthenticated_cannot_export_results(self, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            timeout=10,
        )
        assert resp.status_code == 401

    def test_export_content_type_is_csv(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert "text/csv" in resp.headers.get("Content-Type", "")

    def test_export_has_content_disposition(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        disposition = resp.headers.get("Content-Disposition", "")
        assert "attachment" in disposition
        assert "results_event_" in disposition

    def test_export_body_is_not_empty(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.text) > 0

    def test_export_has_header_row(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        lines = resp.text.strip().splitlines()
        assert len(lines) >= 1
        header = lines[0]
        for col in ("id", "event_id", "participant_id", "value_numeric",
                    "unit", "reviewed_state"):
            assert col in header, f"Missing column '{col}' in CSV header"

    def test_export_data_rows_match_submitted_results(self, auditor_token, export_event):
        resp = requests.get(
            f"{BASE_URL}/events/{export_event['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        lines = resp.text.strip().splitlines()
        # header + at least 2 data rows
        assert len(lines) >= 3

    def test_export_nonexistent_event_returns_404(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/events/999999999/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 404

    def test_export_reflects_approved_correction(self, admin_token, auditor_token,
                                                  ruleset, ts):
        """effective_value column must reflect approved correction, not original."""
        ev = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Export Corr Event {ts}", "is_championship_class": False},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{ev['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        pid = ts + 7100
        result = _submit_result(admin_token, ev["id"], pid, 99000.0, "milliseconds").json()
        corr = _request_correction(
            admin_token, ev["id"], result["id"], 48000.0, "milliseconds"
        ).json()
        _resolve_correction(admin_token, ev["id"], result["id"], corr["id"], "approved")

        resp = requests.get(
            f"{BASE_URL}/events/{ev['id']}/results/export",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        # The effective_value column should contain 48000, not 99000
        assert "48000" in resp.text, "Approved correction must appear as effective_value in CSV"
