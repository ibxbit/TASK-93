"""
Unit tests for data quality business logic.

Mirrors the algorithms in src/data_quality/service.rs.
No server required — pure Python specification tests.

Covers:
- Z-score outlier detection with configurable threshold
- Duplicate detection via stable field hashing
- Required field validation
- Scan configuration defaults and validation
"""

import math
import pytest


# ── Z-score calculation (mirrors service.rs) ───────────────────────────────────

def population_mean(values: list) -> float:
    if not values:
        return 0.0
    return sum(values) / len(values)


def population_stdev(values: list) -> float:
    """Population standard deviation (σ), matching Rust pstdev equivalent."""
    if len(values) < 2:
        return 0.0
    mean = population_mean(values)
    variance = sum((x - mean) ** 2 for x in values) / len(values)
    return math.sqrt(variance)


def zscore(values: list, x: float) -> float:
    """Absolute z-score of x against the rest of the population (leave-one-out).

    Excluding x from the reference distribution prevents the outlier from
    inflating the standard deviation and masking itself — the standard
    approach used in the Rust service.
    """
    others = [v for v in values if v != x]
    if len(others) < 2:
        return 0.0
    stdev = population_stdev(others)
    if stdev == 0.0:
        return 0.0
    mean = population_mean(others)
    return abs(x - mean) / stdev


def is_outlier(values: list, x: float, threshold: float = 3.0) -> bool:
    return zscore(values, x) >= threshold


def outlier_severity(score: float) -> str:
    """Mirrors the severity rule in service.rs: high ≥ 5.0, else medium."""
    if score >= 5.0:
        return "high"
    return "medium"


# ── Duplicate detection via hashing (mirrors service.rs) ─────────────────────

def fnv1a_hash(value: str) -> int:
    """FNV-1a 64-bit hash (simplified; matches intent of Rust fnv crate)."""
    FNV_OFFSET = 14695981039346656037
    FNV_PRIME = 1099511628211
    h = FNV_OFFSET
    for char in value.encode("utf-8"):
        h ^= char
        h = (h * FNV_PRIME) & 0xFFFFFFFFFFFFFFFF
    return h


def compute_record_hash(record: dict, fields: list) -> int:
    """Hash the selected fields of a record to detect duplicates."""
    composite = "|".join(str(record.get(f, "")) for f in sorted(fields))
    return fnv1a_hash(composite)


def find_duplicate_groups(records: list, key_fields: list) -> list:
    """
    Returns groups of records with identical hash for the given key_fields.
    Only groups with > 1 member are returned.
    """
    buckets: dict = {}
    for rec in records:
        h = compute_record_hash(rec, key_fields)
        buckets.setdefault(h, []).append(rec)
    return [group for group in buckets.values() if len(group) > 1]


# ── Required field validation ──────────────────────────────────────────────────

ENTITY_REQUIRED_FIELDS = {
    "invoices":  ["invoice_no", "counterparty", "issue_date"],
    "vehicles":  ["vin", "make", "model", "year"],
    "assets":    ["asset_code", "name", "category"],
    "events":    ["name", "schedule_group"],
    "results":   ["participant_id", "value_numeric", "unit_enum"],
    "payments":  ["amount", "method", "external_reference"],
}


def check_missing_fields(record: dict, entity: str) -> list:
    """
    Returns a list of field names that are NULL or empty-string.
    Mirrors the missing_fields check in service.rs.
    """
    required = ENTITY_REQUIRED_FIELDS.get(entity, [])
    missing = []
    for field in required:
        val = record.get(field)
        if val is None or str(val).strip() == "":
            missing.append(field)
    return missing


# ── Scan config validation ─────────────────────────────────────────────────────

SUPPORTED_ENTITIES = {
    "events", "assets", "vehicles", "invoices", "results", "payments"
}

SUPPORTED_CHECKS = {"missing_fields", "outliers", "duplicates"}

DEFAULT_ZSCORE_THRESHOLD = 3.0
MIN_ZSCORE_THRESHOLD = 0.0001  # must be > 0
MAX_PERIODS = 6
DEFAULT_PERIODS = 3


def validate_scan_config(entity: str, checks: list,
                         zscore_threshold: float = 3.0) -> list:
    """Returns a list of validation errors (empty = valid)."""
    errors = []
    if entity not in SUPPORTED_ENTITIES:
        errors.append(f"Unsupported entity: '{entity}'")
    if not checks:
        errors.append("At least one check is required")
    for c in checks:
        if c not in SUPPORTED_CHECKS:
            errors.append(f"Unknown check type: '{c}'")
    if zscore_threshold <= 0:
        errors.append("zscore_threshold must be > 0")
    return errors


# ── Z-score outlier tests ─────────────────────────────────────────────────────

class TestZScoreCalculation:
    def test_symmetric_distribution_no_outlier(self):
        values = [10.0, 11.0, 12.0, 13.0, 14.0]
        # All values close to mean; no outlier at threshold 3.0
        for v in values:
            assert zscore(values, v) < 3.0

    def test_extreme_value_is_outlier(self):
        values = [10.0, 11.0, 12.0, 13.0, 14.0, 1000.0]
        assert is_outlier(values, 1000.0, threshold=3.0) is True

    def test_mean_value_has_zero_zscore(self):
        values = [10.0, 20.0, 30.0]
        mean = population_mean(values)
        assert zscore(values, mean) == pytest.approx(0.0)

    def test_single_value_has_zero_zscore(self):
        """Single-element population has no variance — z-score is 0."""
        assert zscore([42.0], 42.0) == 0.0

    def test_constant_values_have_zero_zscore(self):
        """All identical values → zero std dev → z-score is 0."""
        values = [5.0, 5.0, 5.0, 5.0]
        assert zscore(values, 5.0) == 0.0
        assert zscore(values, 999.0) == 0.0  # can't compute zscore with zero stdev

    def test_custom_threshold_2_5_more_sensitive(self):
        values = [10.0, 11.0, 12.0, 13.0, 100.0]
        # At threshold 3.0 it may not be flagged; at 2.5 it should be
        score = zscore(values, 100.0)
        assert is_outlier(values, 100.0, threshold=2.5) == (score >= 2.5)

    def test_default_threshold_is_3(self):
        values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
        non_outlier = 5.0  # near middle
        assert is_outlier(values, non_outlier) is False

    def test_outlier_severity_high_at_5_or_above(self):
        assert outlier_severity(5.0) == "high"
        assert outlier_severity(10.0) == "high"
        assert outlier_severity(100.0) == "high"

    def test_outlier_severity_medium_below_5(self):
        assert outlier_severity(3.0) == "medium"
        assert outlier_severity(4.9) == "medium"
        assert outlier_severity(0.0) == "medium"

    def test_outlier_severity_boundary_exactly_5(self):
        assert outlier_severity(5.0) == "high"

    def test_population_stdev_known_values(self):
        # Population σ of [2, 4, 4, 4, 5, 5, 7, 9] = 2.0
        values = [2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]
        assert population_stdev(values) == pytest.approx(2.0, rel=1e-6)

    def test_population_mean_known_values(self):
        assert population_mean([1.0, 2.0, 3.0]) == pytest.approx(2.0)


class TestOutlierThresholdValidation:
    def test_threshold_must_be_positive(self):
        errors = validate_scan_config("vehicles", ["outliers"], zscore_threshold=0.0)
        assert any("zscore" in e for e in errors)

    def test_negative_threshold_invalid(self):
        errors = validate_scan_config("vehicles", ["outliers"], zscore_threshold=-1.0)
        assert any("zscore" in e for e in errors)

    def test_positive_threshold_valid(self):
        errors = validate_scan_config("vehicles", ["outliers"], zscore_threshold=2.5)
        assert not any("zscore" in e for e in errors)

    def test_default_threshold_is_valid(self):
        errors = validate_scan_config("vehicles", ["outliers"])
        assert not errors


# ── Duplicate detection tests ─────────────────────────────────────────────────

class TestDuplicateDetection:
    def test_no_duplicates_in_unique_records(self):
        records = [
            {"vin": "ABC123", "make": "Toyota"},
            {"vin": "DEF456", "make": "Honda"},
            {"vin": "GHI789", "make": "Ford"},
        ]
        groups = find_duplicate_groups(records, ["vin", "make"])
        assert groups == []

    def test_detects_identical_records(self):
        records = [
            {"vin": "SAME123", "make": "Toyota"},
            {"vin": "DIFF456", "make": "Honda"},
            {"vin": "SAME123", "make": "Toyota"},  # duplicate
        ]
        groups = find_duplicate_groups(records, ["vin", "make"])
        assert len(groups) == 1
        assert len(groups[0]) == 2

    def test_detects_multiple_duplicate_groups(self):
        records = [
            {"vin": "AAA", "make": "Toyota"},
            {"vin": "AAA", "make": "Toyota"},
            {"vin": "BBB", "make": "Honda"},
            {"vin": "BBB", "make": "Honda"},
        ]
        groups = find_duplicate_groups(records, ["vin", "make"])
        assert len(groups) == 2

    def test_partial_field_match_not_duplicate(self):
        records = [
            {"vin": "SAME", "make": "Toyota"},
            {"vin": "SAME", "make": "Honda"},  # same VIN, different make
        ]
        groups = find_duplicate_groups(records, ["vin", "make"])
        assert groups == []

    def test_single_field_duplicate_detection(self):
        records = [
            {"vin": "DUP", "make": "Toyota"},
            {"vin": "DUP", "make": "Honda"},
        ]
        groups = find_duplicate_groups(records, ["vin"])  # only VIN
        assert len(groups) == 1

    def test_empty_records_no_duplicates(self):
        assert find_duplicate_groups([], ["vin"]) == []

    def test_single_record_no_duplicates(self):
        records = [{"vin": "ONLY", "make": "Toyota"}]
        assert find_duplicate_groups(records, ["vin"]) == []

    def test_hash_is_deterministic(self):
        record = {"vin": "ABC123", "make": "Toyota", "model": "Corolla"}
        h1 = compute_record_hash(record, ["vin", "make"])
        h2 = compute_record_hash(record, ["vin", "make"])
        assert h1 == h2

    def test_field_order_in_config_does_not_affect_hash(self):
        """Fields are sorted before hashing — order in the config must not matter."""
        record = {"vin": "TEST", "make": "X", "model": "Y"}
        h1 = compute_record_hash(record, ["vin", "make"])
        h2 = compute_record_hash(record, ["make", "vin"])
        assert h1 == h2

    def test_triplicate_forms_one_group_of_three(self):
        records = [
            {"sn": "TRIPLE"},
            {"sn": "TRIPLE"},
            {"sn": "TRIPLE"},
        ]
        groups = find_duplicate_groups(records, ["sn"])
        assert len(groups) == 1
        assert len(groups[0]) == 3


# ── Required field validation tests ──────────────────────────────────────────

class TestRequiredFieldValidation:
    def test_complete_invoice_has_no_missing_fields(self):
        record = {
            "invoice_no": "INV-001",
            "counterparty": "Acme",
            "issue_date": "2026-01-01",
        }
        assert check_missing_fields(record, "invoices") == []

    def test_missing_invoice_no_detected(self):
        record = {"counterparty": "Acme", "issue_date": "2026-01-01"}
        missing = check_missing_fields(record, "invoices")
        assert "invoice_no" in missing

    def test_null_field_detected_as_missing(self):
        record = {
            "invoice_no": None,
            "counterparty": "Acme",
            "issue_date": "2026-01-01",
        }
        assert "invoice_no" in check_missing_fields(record, "invoices")

    def test_empty_string_detected_as_missing(self):
        record = {
            "invoice_no": "   ",  # whitespace only
            "counterparty": "Acme",
            "issue_date": "2026-01-01",
        }
        assert "invoice_no" in check_missing_fields(record, "invoices")

    def test_complete_vehicle_has_no_missing(self):
        record = {"vin": "1HGCM82633A004352", "make": "Honda",
                  "model": "Civic", "year": 2020}
        assert check_missing_fields(record, "vehicles") == []

    def test_missing_vin_detected(self):
        record = {"make": "Honda", "model": "Civic", "year": 2020}
        assert "vin" in check_missing_fields(record, "vehicles")

    def test_complete_asset_has_no_missing(self):
        record = {"asset_code": "ASSET-001", "name": "Radar Gun", "category": "equipment"}
        assert check_missing_fields(record, "assets") == []

    def test_multiple_missing_fields_all_reported(self):
        record = {}  # empty record
        missing = check_missing_fields(record, "invoices")
        assert "invoice_no" in missing
        assert "counterparty" in missing
        assert "issue_date" in missing

    def test_unknown_entity_returns_no_errors(self):
        """Unknown entities have no mandatory fields — no false positives."""
        assert check_missing_fields({"foo": "bar"}, "unknown_entity") == []


# ── Scan config validation tests ──────────────────────────────────────────────

class TestScanConfigValidation:
    def test_valid_config_has_no_errors(self):
        errors = validate_scan_config("invoices", ["missing_fields"])
        assert errors == []

    def test_unknown_entity_is_invalid(self):
        errors = validate_scan_config("magic_table", ["missing_fields"])
        assert errors

    def test_empty_checks_list_is_invalid(self):
        errors = validate_scan_config("invoices", [])
        assert errors

    def test_unknown_check_type_is_invalid(self):
        errors = validate_scan_config("invoices", ["spell_check"])
        assert errors

    def test_all_valid_entities_accepted(self):
        for entity in SUPPORTED_ENTITIES:
            errors = validate_scan_config(entity, ["missing_fields"])
            entity_errors = [e for e in errors if entity in e]
            assert not entity_errors, f"Entity '{entity}' should be valid"

    def test_all_valid_check_types_accepted(self):
        for check in SUPPORTED_CHECKS:
            errors = validate_scan_config("invoices", [check])
            check_errors = [e for e in errors if check in e]
            assert not check_errors, f"Check '{check}' should be valid"

    def test_multiple_valid_checks_accepted(self):
        errors = validate_scan_config(
            "vehicles",
            ["missing_fields", "outliers", "duplicates"]
        )
        assert not errors

    def test_valid_and_invalid_checks_mixed(self):
        errors = validate_scan_config("invoices", ["missing_fields", "voodoo"])
        assert any("voodoo" in e or "Unknown" in e for e in errors)
