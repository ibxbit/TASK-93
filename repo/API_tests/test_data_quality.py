"""
API tests for the data quality module.

Covers:
- POST /data-quality/scans  (run scan; audit:read)
- GET  /data-quality/scans  (list scans; audit:read)
- GET  /data-quality/scans/<id>  (get full report; audit:read)
- All three check types: missing_fields, outliers, duplicates
- Configurable z-score threshold
- Custom numeric_fields and hash_fields
- RBAC enforcement
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Helpers ───────────────────────────────────────────────────────────────────

def _run_scan(token, entity, checks, **kwargs):
    """POST /data-quality/scans and return the response."""
    payload = {"entity": entity, "checks": checks}
    payload.update(kwargs)
    return requests.post(
        f"{BASE_URL}/data-quality/scans",
        headers=auth_headers(token),
        json=payload,
        timeout=30,
    )


# ── Access control ────────────────────────────────────────────────────────────

class TestDataQualityAccess:
    def test_admin_can_run_scan(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        assert resp.status_code == 200

    def test_auditor_can_run_scan(self, auditor_token):
        resp = _run_scan(auditor_token, "invoices", ["missing_fields"])
        assert resp.status_code == 200

    def test_referee_cannot_run_scan(self, referee_token):
        resp = _run_scan(referee_token, "invoices", ["missing_fields"])
        assert resp.status_code == 403

    def test_finance_clerk_cannot_run_scan(self, finance_token):
        resp = _run_scan(finance_token, "invoices", ["missing_fields"])
        assert resp.status_code == 403

    def test_event_director_cannot_run_scan(self, director_token):
        resp = _run_scan(director_token, "invoices", ["missing_fields"])
        assert resp.status_code == 403

    def test_unauthenticated_returns_401(self):
        resp = requests.post(
            f"{BASE_URL}/data-quality/scans",
            json={"entity": "invoices", "checks": ["missing_fields"]},
            timeout=10,
        )
        assert resp.status_code == 401


# ── Scan response shape ───────────────────────────────────────────────────────

class TestScanResponseShape:
    def test_scan_report_has_required_fields(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        assert resp.status_code == 200
        body = resp.json()
        required = {
            "id", "entity", "config", "total_records",
            "anomaly_count", "anomalies", "created_by", "created_at",
        }
        assert required.issubset(body.keys())

    def test_scan_config_reflects_request(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        body = resp.json()
        assert "missing_fields" in body["config"]["checks_run"]

    def test_scan_anomalies_is_list(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        assert isinstance(resp.json()["anomalies"], list)

    def test_scan_total_records_is_non_negative(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        assert resp.json()["total_records"] >= 0

    def test_scan_anomaly_count_matches_list_length(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        body = resp.json()
        assert body["anomaly_count"] == len(body["anomalies"])

    def test_anomaly_has_required_fields(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["missing_fields"])
        body = resp.json()
        for anomaly in body["anomalies"]:
            assert "check" in anomaly
            assert "record_id" in anomaly
            assert "detail" in anomaly
            assert "severity" in anomaly

    def test_anomaly_severity_is_valid_value(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["missing_fields"])
        valid_severities = {"low", "medium", "high"}
        for anomaly in resp.json()["anomalies"]:
            assert anomaly["severity"] in valid_severities


# ── Missing fields check ──────────────────────────────────────────────────────

class TestMissingFieldsCheck:
    def test_missing_fields_check_on_invoices(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["missing_fields"])
        assert resp.status_code == 200
        body = resp.json()
        assert "missing_fields" in body["config"]["checks_run"]

    def test_missing_fields_check_on_vehicles(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["missing_fields"])
        assert resp.status_code == 200

    def test_missing_fields_check_on_assets(self, admin_token):
        resp = _run_scan(admin_token, "assets", ["missing_fields"])
        assert resp.status_code == 200

    def test_missing_fields_check_on_events(self, admin_token):
        resp = _run_scan(admin_token, "events", ["missing_fields"])
        assert resp.status_code == 200

    def test_missing_fields_check_on_results(self, admin_token):
        resp = _run_scan(admin_token, "results", ["missing_fields"])
        assert resp.status_code == 200

    def test_missing_fields_check_on_payments(self, admin_token):
        resp = _run_scan(admin_token, "payments", ["missing_fields"])
        assert resp.status_code == 200

    def test_missing_fields_anomalies_tagged_correctly(self, admin_token):
        body = _run_scan(admin_token, "invoices", ["missing_fields"]).json()
        for anomaly in body["anomalies"]:
            if anomaly["check"] == "missing_fields":
                assert anomaly["field"] is not None
                assert anomaly["severity"] == "high"
                break

    def test_unknown_entity_returns_400(self, admin_token):
        resp = _run_scan(admin_token, "nonexistent_table", ["missing_fields"])
        assert resp.status_code == 400


# ── Outlier detection ─────────────────────────────────────────────────────────

class TestOutlierDetection:
    def test_outlier_check_on_vehicles(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"])
        assert resp.status_code == 200
        body = resp.json()
        assert "outliers" in body["config"]["checks_run"]

    def test_outlier_check_on_assets(self, admin_token):
        resp = _run_scan(admin_token, "assets", ["outliers"])
        assert resp.status_code == 200

    def test_outlier_check_on_invoices(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["outliers"])
        assert resp.status_code == 200

    def test_custom_zscore_threshold_accepted(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"],
                         zscore_threshold=2.5)
        assert resp.status_code == 200
        assert resp.json()["config"]["zscore_threshold"] == 2.5

    def test_default_zscore_threshold_is_3(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"])
        assert resp.status_code == 200
        assert resp.json()["config"]["zscore_threshold"] == 3.0

    def test_zero_zscore_threshold_rejected(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"],
                         zscore_threshold=0.0)
        assert resp.status_code == 400

    def test_negative_zscore_threshold_rejected(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"],
                         zscore_threshold=-1.0)
        assert resp.status_code == 400

    def test_outlier_anomaly_has_score(self, admin_token):
        body = _run_scan(admin_token, "vehicles", ["outliers"]).json()
        for anomaly in body["anomalies"]:
            if anomaly["check"] == "outliers":
                assert anomaly["score"] is not None
                assert isinstance(anomaly["score"], (int, float))
                break

    def test_outlier_anomaly_tagged_check_is_outliers(self, admin_token):
        body = _run_scan(admin_token, "vehicles", ["outliers"]).json()
        for anomaly in body["anomalies"]:
            assert anomaly["check"] in {"outliers", "missing_fields", "duplicates"}

    def test_custom_numeric_fields_accepted(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"],
                         numeric_fields=["year", "mileage"])
        assert resp.status_code == 200
        config = resp.json()["config"]
        assert "year" in config["numeric_fields"]
        assert "mileage" in config["numeric_fields"]

    def test_invalid_numeric_field_returns_400(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["outliers"],
                         numeric_fields=["nonexistent_column"])
        assert resp.status_code == 400

    def test_outlier_severity_is_high_or_medium(self, admin_token):
        body = _run_scan(admin_token, "vehicles", ["outliers"]).json()
        for anomaly in body["anomalies"]:
            if anomaly["check"] == "outliers":
                assert anomaly["severity"] in {"medium", "high"}


# ── Duplicate detection ───────────────────────────────────────────────────────

class TestDuplicateDetection:
    def test_duplicate_check_on_vehicles(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["duplicates"])
        assert resp.status_code == 200
        assert "duplicates" in resp.json()["config"]["checks_run"]

    def test_duplicate_check_on_assets(self, admin_token):
        resp = _run_scan(admin_token, "assets", ["duplicates"])
        assert resp.status_code == 200

    def test_duplicate_check_on_invoices(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["duplicates"])
        assert resp.status_code == 200

    def test_custom_hash_fields_accepted(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["duplicates"],
                         hash_fields=["vin", "make"])
        assert resp.status_code == 200
        config = resp.json()["config"]
        assert "vin" in config["hash_fields"]

    def test_invalid_hash_field_returns_400(self, admin_token):
        resp = _run_scan(admin_token, "vehicles", ["duplicates"],
                         hash_fields=["definitely_not_a_column"])
        assert resp.status_code == 400

    def test_duplicate_anomalies_have_no_score(self, admin_token):
        body = _run_scan(admin_token, "vehicles", ["duplicates"]).json()
        for anomaly in body["anomalies"]:
            if anomaly["check"] == "duplicates":
                # duplicate anomalies don't have z-score
                assert anomaly.get("score") is None
                break


# ── Combined checks ───────────────────────────────────────────────────────────

class TestCombinedChecks:
    def test_all_checks_together(self, admin_token):
        resp = _run_scan(admin_token, "vehicles",
                         ["missing_fields", "outliers", "duplicates"])
        assert resp.status_code == 200
        config = resp.json()["config"]
        assert "missing_fields" in config["checks_run"]
        assert "outliers" in config["checks_run"]
        assert "duplicates" in config["checks_run"]

    def test_missing_fields_and_outliers(self, admin_token):
        resp = _run_scan(admin_token, "assets",
                         ["missing_fields", "outliers"])
        assert resp.status_code == 200

    def test_empty_checks_list_returns_400(self, admin_token):
        resp = _run_scan(admin_token, "invoices", [])
        assert resp.status_code == 400

    def test_unknown_check_type_returns_400(self, admin_token):
        resp = _run_scan(admin_token, "invoices", ["magic_check"])
        assert resp.status_code == 400


# ── Scan listing and retrieval ────────────────────────────────────────────────

class TestScanList:
    def test_list_scans_returns_array(self, admin_token):
        # Ensure at least one scan exists first
        _run_scan(admin_token, "invoices", ["missing_fields"])
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_scan_summary_has_required_fields(self, admin_token):
        _run_scan(admin_token, "invoices", ["missing_fields"])
        summaries = requests.get(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        if summaries:
            s = summaries[0]
            required = {
                "id", "entity", "checks_run", "total_records",
                "anomaly_count", "created_by", "created_at",
            }
            assert required.issubset(s.keys())

    def test_filter_by_entity(self, admin_token):
        _run_scan(admin_token, "invoices", ["missing_fields"])
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans?entity=invoices",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        for s in resp.json():
            assert s["entity"] == "invoices"

    def test_list_limit_default_is_20(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.json()) <= 20

    def test_list_limit_custom(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans?limit=5",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.json()) <= 5

    def test_list_limit_max_100(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans?limit=9999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.json()) <= 100

    def test_auditor_can_list_scans(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_list_scans(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403


class TestScanGet:
    def test_get_scan_by_id(self, admin_token):
        scan_id = _run_scan(admin_token, "invoices", ["missing_fields"]).json()["id"]
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans/{scan_id}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200
        body = resp.json()
        assert body["id"] == scan_id

    def test_get_scan_includes_full_anomaly_list(self, admin_token):
        scan = _run_scan(admin_token, "vehicles", ["missing_fields"]).json()
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans/{scan['id']}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        # Full report has anomaly details, not just summary
        assert "anomalies" in resp
        assert len(resp["anomalies"]) == resp["anomaly_count"]

    def test_get_nonexistent_scan_returns_404(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans/999999999",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 404

    def test_auditor_can_get_scan(self, auditor_token):
        scan_id = _run_scan(auditor_token, "invoices", ["missing_fields"]).json()["id"]
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans/{scan_id}",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_get_scan(self, referee_token, admin_token):
        scan_id = _run_scan(admin_token, "invoices", ["missing_fields"]).json()["id"]
        resp = requests.get(
            f"{BASE_URL}/data-quality/scans/{scan_id}",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403
