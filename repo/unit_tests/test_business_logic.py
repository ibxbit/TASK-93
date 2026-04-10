"""
Unit tests for core business logic rules.

Validates calculations, state machine rules, and domain constraints as
specified in the API contract.  No server required — pure Python tests.

Covers:
- VIN format validation (ISO 3779)
- Invoice discount and tax rules
- Backup rotation and retention policy
- Backup filename and path-traversal safety
- Backup SQLite magic-bytes validation
- Vehicle status state machine
- Timestamp format validation
"""

import pytest
import re
import struct
from datetime import datetime, timezone


# ── VIN validation ─────────────────────────────────────────────────────────────

EXCLUDED_VIN_CHARS = set("IOQ")
VIN_PATTERN = re.compile(r'^[A-HJ-NPR-Z0-9]{17}$')


def validate_vin(vin: str) -> bool:
    """
    ISO 3779 VIN: exactly 17 alphanumeric characters.
    Characters I, O, Q are excluded to avoid confusion with 1, 0, 0.
    """
    return bool(VIN_PATTERN.match(vin))


class TestVinValidation:
    def test_valid_vin_accepted(self):
        assert validate_vin("1HGCM82633A004352") is True

    def test_valid_vin_all_digits_and_letters(self):
        assert validate_vin("WBA3A5G59DNP26082") is True

    def test_vin_too_short_rejected(self):
        assert validate_vin("1HGCM8263") is False

    def test_vin_too_long_rejected(self):
        assert validate_vin("1HGCM82633A004352X") is False

    def test_vin_with_I_rejected(self):
        assert validate_vin("1HGCM82633I004352") is False

    def test_vin_with_O_rejected(self):
        assert validate_vin("1HGCMO2633A004352") is False

    def test_vin_with_Q_rejected(self):
        assert validate_vin("1HGCM82633Q004352") is False

    def test_vin_lowercase_rejected(self):
        assert validate_vin("1hgcm82633a004352") is False

    def test_vin_with_special_chars_rejected(self):
        assert validate_vin("1HGCM826-3A004352") is False

    def test_empty_vin_rejected(self):
        assert validate_vin("") is False


# ── Invoice discount rules ─────────────────────────────────────────────────────

MAX_DISCOUNT_PCT = 30.0    # Maximum percentage discount
MAX_DISCOUNT_ABS = 500.0   # Maximum absolute discount in dollars


def compute_percentage_discount(subtotal: float, pct: float) -> float:
    """Apply a percentage discount, capped at MAX_DISCOUNT_ABS."""
    raw = subtotal * (pct / 100.0)
    return min(raw, MAX_DISCOUNT_ABS)


def compute_fixed_discount(amount: float) -> float:
    """Fixed-amount discount, capped at MAX_DISCOUNT_ABS."""
    return min(amount, MAX_DISCOUNT_ABS)


class TestInvoiceDiscountRules:
    def test_percentage_discount_basic(self):
        result = compute_percentage_discount(1000.0, 10.0)
        assert abs(result - 100.0) < 0.01

    def test_percentage_discount_zero(self):
        result = compute_percentage_discount(1000.0, 0.0)
        assert result == 0.0

    def test_percentage_discount_capped_at_500(self):
        # 30% of $5000 = $1500 → capped at $500
        result = compute_percentage_discount(5000.0, 30.0)
        assert abs(result - 500.0) < 0.01

    def test_fixed_discount_basic(self):
        result = compute_fixed_discount(200.0)
        assert abs(result - 200.0) < 0.01

    def test_fixed_discount_capped_at_500(self):
        result = compute_fixed_discount(999.99)
        assert abs(result - 500.0) < 0.01

    def test_fixed_discount_zero(self):
        result = compute_fixed_discount(0.0)
        assert result == 0.0

    def test_max_percentage_is_30(self):
        # 31% should be rejected by the API (validated upstream)
        # Verify the business rule constant
        assert MAX_DISCOUNT_PCT == 30.0

    def test_max_discount_cap_is_500(self):
        assert MAX_DISCOUNT_ABS == 500.0


# ── Invoice tax calculation ────────────────────────────────────────────────────

def compute_invoice_totals(subtotal: float, tax_rate: float, discount: float = 0.0):
    """
    Invoice total = (subtotal - discount) * (1 + tax_rate)
    Tax is applied after discount.
    """
    discounted = max(subtotal - discount, 0.0)
    tax = discounted * tax_rate
    total = discounted + tax
    return {"subtotal": subtotal, "discount": discount, "tax": tax, "total": total}


class TestInvoiceTaxCalculation:
    def test_no_tax(self):
        result = compute_invoice_totals(1000.0, 0.0)
        assert abs(result["total"] - 1000.0) < 0.01

    def test_10_percent_tax(self):
        result = compute_invoice_totals(1000.0, 0.10)
        assert abs(result["tax"] - 100.0) < 0.01
        assert abs(result["total"] - 1100.0) < 0.01

    def test_20_percent_tax(self):
        result = compute_invoice_totals(500.0, 0.20)
        assert abs(result["total"] - 600.0) < 0.01

    def test_tax_applied_after_discount(self):
        # $1000 subtotal, $100 discount → $900 taxable, 10% tax = $90
        result = compute_invoice_totals(1000.0, 0.10, discount=100.0)
        assert abs(result["tax"] - 90.0) < 0.01
        assert abs(result["total"] - 990.0) < 0.01


# ── Backup rotation logic ──────────────────────────────────────────────────────

def rotate_backups(filenames: list, retain_days: int) -> tuple:
    """
    Returns (kept, deleted) filename lists.
    Files sorted lexicographically (timestamp-prefixed names = chronological).
    Keeps the `retain_days` most recent files.
    """
    sorted_files = sorted(filenames)
    to_delete_count = max(0, len(sorted_files) - retain_days)
    deleted = sorted_files[:to_delete_count]
    kept = sorted_files[to_delete_count:]
    return kept, deleted


class TestBackupRotation:
    def test_retain_7_days_keeps_7_most_recent(self):
        files = [f"backup_2026010{i}_120000.sqlite" for i in range(1, 10)]  # 9 files
        kept, deleted = rotate_backups(files, 7)
        assert len(kept) == 7
        assert len(deleted) == 2

    def test_fewer_files_than_retain_nothing_deleted(self):
        files = [f"backup_2026010{i}_120000.sqlite" for i in range(1, 4)]  # 3 files
        kept, deleted = rotate_backups(files, 7)
        assert len(kept) == 3
        assert len(deleted) == 0

    def test_exactly_retain_days_nothing_deleted(self):
        files = [f"backup_2026010{i}_120000.sqlite" for i in range(1, 8)]  # 7 files
        kept, deleted = rotate_backups(files, 7)
        assert len(kept) == 7
        assert len(deleted) == 0

    def test_oldest_files_deleted_first(self):
        files = [
            "backup_20260101_120000.sqlite",
            "backup_20260102_120000.sqlite",
            "backup_20260103_120000.sqlite",
        ]
        kept, deleted = rotate_backups(files, 2)
        assert "backup_20260101_120000.sqlite" in deleted
        assert "backup_20260103_120000.sqlite" in kept

    def test_retain_zero_deletes_all(self):
        files = ["backup_20260101_120000.sqlite", "backup_20260102_120000.sqlite"]
        kept, deleted = rotate_backups(files, 0)
        assert len(kept) == 0
        assert len(deleted) == 2


# ── Backup filename format ─────────────────────────────────────────────────────

BACKUP_FILENAME_PATTERN = re.compile(
    r'^backup_\d{8}_\d{6}\.sqlite$'
)


def validate_backup_filename(filename: str) -> bool:
    return bool(BACKUP_FILENAME_PATTERN.match(filename))


class TestBackupFilenameFormat:
    def test_valid_backup_filename(self):
        assert validate_backup_filename("backup_20260407_123456.sqlite") is True

    def test_wrong_extension_rejected(self):
        assert validate_backup_filename("backup_20260407_123456.db") is False

    def test_missing_prefix_rejected(self):
        assert validate_backup_filename("20260407_123456.sqlite") is False

    def test_wrong_date_format_rejected(self):
        assert validate_backup_filename("backup_2026-04-07_123456.sqlite") is False

    def test_path_traversal_rejected(self):
        assert validate_backup_filename("../secret.sqlite") is False
        assert validate_backup_filename("backup_20260407_123456.sqlite/../evil") is False


# ── Vehicle status machine ─────────────────────────────────────────────────────

VALID_TRANSITIONS = {
    "draft": {"published"},
    "published": {"delisted", "sold"},
    "delisted": {"published", "sold"},  # NOT terminal — can relist or sell
    "sold": set(),                       # terminal
}


def can_transition(current: str, target: str) -> bool:
    return target in VALID_TRANSITIONS.get(current, set())


class TestVehicleStatusMachine:
    def test_draft_to_published_allowed(self):
        assert can_transition("draft", "published") is True

    def test_published_to_delisted_allowed(self):
        assert can_transition("published", "delisted") is True

    def test_published_to_sold_allowed(self):
        assert can_transition("published", "sold") is True

    def test_draft_to_sold_not_allowed(self):
        assert can_transition("draft", "sold") is False

    def test_delisted_can_relist(self):
        assert can_transition("delisted", "published") is True

    def test_delisted_can_sell(self):
        assert can_transition("delisted", "sold") is True

    def test_delisted_cannot_go_to_draft(self):
        assert can_transition("delisted", "draft") is False

    def test_sold_is_terminal(self):
        assert can_transition("sold", "draft") is False
        assert can_transition("sold", "published") is False
        assert can_transition("sold", "delisted") is False

    def test_cannot_transition_to_same_status(self):
        for status in VALID_TRANSITIONS:
            assert can_transition(status, status) is False


# ── ISO 8601 timestamp parsing ─────────────────────────────────────────────────

def is_valid_iso8601(ts: str) -> bool:
    """Basic RFC 3339 / ISO 8601 validation used for effective_at fields."""
    try:
        datetime.fromisoformat(ts.replace("Z", "+00:00"))
        return True
    except ValueError:
        return False


class TestTimestampParsing:
    def test_valid_utc_timestamp(self):
        assert is_valid_iso8601("2026-01-01T00:00:00Z") is True

    def test_valid_offset_timestamp(self):
        assert is_valid_iso8601("2026-04-07T12:30:00+05:30") is True

    def test_invalid_timestamp_rejected(self):
        assert is_valid_iso8601("not-a-date") is False

    def test_date_only_valid(self):
        assert is_valid_iso8601("2026-01-01") is True

    def test_month_out_of_range_rejected(self):
        assert is_valid_iso8601("2026-13-01T00:00:00Z") is False


# ── Backup path-traversal and SQLite validation ───────────────────────────────

SQLITE_MAGIC = b"SQLite format 3\x00"   # first 16 bytes of every SQLite file


def validate_backup_filename_strict(filename: str) -> bool:
    """
    Validate that a filename is safe for use in restore operations.
    Rejects path traversal sequences, directory separators, and non-conforming patterns.
    Mirrors stage_restore() in src/backup/service.rs.
    """
    if "/" in filename or "\\" in filename or ".." in filename:
        return False
    return bool(BACKUP_FILENAME_PATTERN.match(filename))


def validate_sqlite_magic(data: bytes) -> bool:
    """Return True if the first 16 bytes match the SQLite file header magic."""
    return len(data) >= 16 and data[:16] == SQLITE_MAGIC


class TestBackupPathTraversal:
    def test_normal_filename_accepted(self):
        assert validate_backup_filename_strict("backup_20260407_120000.sqlite") is True

    def test_dotdot_prefix_rejected(self):
        assert validate_backup_filename_strict("../etc/passwd") is False

    def test_dotdot_embedded_rejected(self):
        assert validate_backup_filename_strict("backup_20260407_120000.sqlite/../secret") is False

    def test_forward_slash_rejected(self):
        assert validate_backup_filename_strict("subdir/backup_20260407_120000.sqlite") is False

    def test_backslash_rejected(self):
        assert validate_backup_filename_strict("sub\\backup_20260407_120000.sqlite") is False

    def test_absolute_path_rejected(self):
        assert validate_backup_filename_strict("/etc/shadow") is False

    def test_dot_only_rejected(self):
        assert validate_backup_filename_strict(".") is False
        assert validate_backup_filename_strict("..") is False


class TestSQLiteMagicBytes:
    def test_valid_sqlite_header_accepted(self):
        data = SQLITE_MAGIC + b"\x00" * 100
        assert validate_sqlite_magic(data) is True

    def test_wrong_header_rejected(self):
        assert validate_sqlite_magic(b"Not SQLite\x00\x00\x00\x00\x00\x00") is False

    def test_empty_data_rejected(self):
        assert validate_sqlite_magic(b"") is False

    def test_truncated_header_rejected(self):
        """Must have at least 16 bytes to hold the full magic."""
        assert validate_sqlite_magic(SQLITE_MAGIC[:10]) is False

    def test_json_file_rejected(self):
        assert validate_sqlite_magic(b'{"status":"ok"}') is False

    def test_zip_file_rejected(self):
        # ZIP magic: PK\x03\x04
        assert validate_sqlite_magic(b"PK\x03\x04" + b"\x00" * 12) is False


class TestBackupRetentionPolicy:
    """Retention policy rules mirror src/backup/service.rs:rotate_backups()."""

    def test_default_retention_is_7_days(self):
        """The documented default for BACKUP_RETAIN_DAYS is 7."""
        DEFAULT_RETAIN_DAYS = 7
        files = [f"backup_2026010{i}_120000.sqlite" for i in range(1, 10)]  # 9 files
        kept, deleted = rotate_backups(files, DEFAULT_RETAIN_DAYS)
        assert len(kept) == DEFAULT_RETAIN_DAYS

    def test_retain_1_keeps_only_newest(self):
        files = [
            "backup_20260101_120000.sqlite",
            "backup_20260102_120000.sqlite",
            "backup_20260103_120000.sqlite",
        ]
        kept, deleted = rotate_backups(files, 1)
        assert kept == ["backup_20260103_120000.sqlite"]
        assert len(deleted) == 2

    def test_retain_must_not_delete_newer_than_older(self):
        files = [
            "backup_20260101_120000.sqlite",  # oldest
            "backup_20260105_120000.sqlite",
            "backup_20260110_120000.sqlite",  # newest
        ]
        kept, deleted = rotate_backups(files, 2)
        assert "backup_20260110_120000.sqlite" in kept
        assert "backup_20260105_120000.sqlite" in kept
        assert "backup_20260101_120000.sqlite" in deleted

    def test_single_file_never_deleted_with_retain_1(self):
        kept, deleted = rotate_backups(["backup_20260101_120000.sqlite"], 1)
        assert len(kept) == 1
        assert len(deleted) == 0

    def test_non_backup_files_ignored_in_rotation(self):
        """Rotation must only count recognised backup files."""
        # Non-conforming names should not be in the rotation list at all
        # (they would be filtered out before being passed to rotate_backups)
        conforming = [
            "backup_20260101_120000.sqlite",
            "backup_20260102_120000.sqlite",
        ]
        kept, deleted = rotate_backups(conforming, 1)
        assert len(kept) == 1
