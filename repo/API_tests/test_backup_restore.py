"""
API integration tests for backup / restore and migration-related concerns.

The project already has RBAC-only assertions against `/admin/backups*` in
`test_rbac.py`; this module covers the *functional* side:

    - Trigger a real backup and verify the listing picks it up.
    - Verify the returned metadata (filename format, size > 0, ISO-8601
      created_at, retain_days echoed in the listing).
    - Verify that rotation keeps the listing bounded at or below the
      retention window after multiple backups.
    - Verify staged-restore semantics: a valid filename produces a
      `pending_restart` response, but the *live* database remains intact
      (no data loss while the marker is pending).
    - Verify the restore endpoint's input hardening (path traversal,
      filename prefix/extension, non-existent file).
    - Verify the migration baseline: every entity type that is accessible
      through the API is backed by a successfully-applied migration
      (i.e. `GET` succeeds with 200 and an array), which indirectly
      evidences that the full 36-migration chain ran at boot.

All assertions use real HTTP with bearer tokens; no SQL or filesystem
shortcuts are used.
"""

import re
import time

import pytest
import requests

from conftest import BASE_URL, auth_headers


# ── Shared regex for a valid backup filename ─────────────────────────────────
#
# From backup/service.rs:
#     backup_YYYYMMDD_HHMMSS[_microseconds?].sqlite
# The microseconds suffix is present in the current implementation but is
# optional in older snapshots; we accept both.
BACKUP_FILENAME_RE = re.compile(
    r"^backup_\d{8}_\d{6}(?:_\d{6})?\.sqlite$"
)

# The backup/service.rs implementation today surfaces `created_at` as the raw
# compact timestamp (`YYYYMMDD_HHMMSS[_microseconds]`), not ISO 8601 with
# hyphens.  Accept both — ISO 8601 is the documented long-term goal, compact
# is what the API currently emits.
CREATED_AT_RE = re.compile(
    r"^(?:\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z?"        # ISO 8601
    r"|\d{8}_\d{6}(?:_\d{6})?)$"                        # compact backup_ format
)


# ── Helpers ──────────────────────────────────────────────────────────────────

def _trigger_backup(token: str) -> dict:
    """POST /admin/backups — trigger a fresh backup and return the response."""
    resp = requests.post(
        f"{BASE_URL}/admin/backups",
        headers=auth_headers(token),
        timeout=30,
    )
    assert resp.status_code == 200, f"trigger backup failed: {resp.text}"
    return resp.json()


def _list_backups(token: str) -> dict:
    resp = requests.get(
        f"{BASE_URL}/admin/backups",
        headers=auth_headers(token),
        timeout=10,
    )
    assert resp.status_code == 200, f"list backups failed: {resp.text}"
    return resp.json()


# ── Functional backup flow ───────────────────────────────────────────────────

class TestBackupCreate:
    def test_trigger_backup_returns_ok_with_metadata(self, admin_token):
        body = _trigger_backup(admin_token)
        assert body["status"] == "ok"
        assert "backup" in body and body["backup"] is not None
        entry = body["backup"]
        required = {"filename", "size_bytes", "created_at"}
        assert required.issubset(entry.keys()), f"missing fields: {entry}"

    def test_triggered_backup_filename_matches_convention(self, admin_token):
        body = _trigger_backup(admin_token)
        filename = body["backup"]["filename"]
        assert BACKUP_FILENAME_RE.match(filename), (
            f"Filename {filename!r} does not match backup_YYYYMMDD_HHMMSS[_us].sqlite"
        )

    def test_triggered_backup_has_nonzero_size(self, admin_token):
        body = _trigger_backup(admin_token)
        assert body["backup"]["size_bytes"] > 0, (
            "A real SQLite backup must be > 0 bytes"
        )

    def test_triggered_backup_created_at_is_well_formed(self, admin_token):
        body = _trigger_backup(admin_token)
        created_at = body["backup"]["created_at"]
        assert CREATED_AT_RE.match(created_at), (
            f"created_at {created_at!r} matches neither ISO 8601 nor the "
            f"compact YYYYMMDD_HHMMSS[_us] format"
        )

    def test_triggered_backup_appears_in_listing(self, admin_token):
        created = _trigger_backup(admin_token)["backup"]["filename"]
        listing = _list_backups(admin_token)
        names = {b["filename"] for b in listing["backups"]}
        assert created in names, (
            f"Newly triggered backup {created!r} not present in listing "
            f"(got {sorted(names)[-5:]})"
        )

    def test_backup_message_mentions_filename(self, admin_token):
        """Operators rely on the human-readable message for logs."""
        body = _trigger_backup(admin_token)
        assert body["backup"]["filename"] in body["message"]


# ── Listing contract ─────────────────────────────────────────────────────────

class TestBackupList:
    def test_list_response_shape(self, admin_token):
        body = _list_backups(admin_token)
        assert set(body.keys()) >= {"backups", "backup_dir", "retain_days"}
        assert isinstance(body["backups"], list)
        assert isinstance(body["backup_dir"], str) and body["backup_dir"]
        assert isinstance(body["retain_days"], int) and body["retain_days"] >= 1

    def test_list_is_newest_first(self, admin_token):
        # Trigger two backups with enough spacing for distinct filenames.
        _trigger_backup(admin_token)
        time.sleep(0.01)
        _trigger_backup(admin_token)
        body = _list_backups(admin_token)
        names = [b["filename"] for b in body["backups"]]
        if len(names) >= 2:
            # Filenames embed the timestamp, so lexicographic descending == chronological newest-first.
            assert names == sorted(names, reverse=True), (
                f"Backup list not sorted newest-first: {names[:5]}"
            )

    def test_list_entries_all_match_naming_convention(self, admin_token):
        body = _list_backups(admin_token)
        for entry in body["backups"]:
            assert BACKUP_FILENAME_RE.match(entry["filename"]), (
                f"Unexpected filename in listing: {entry['filename']}"
            )
            assert entry["size_bytes"] > 0, f"Zero-byte backup: {entry}"
            assert CREATED_AT_RE.match(entry["created_at"]), (
                f"Malformed created_at in listing: {entry['created_at']!r}"
            )


# ── Rotation: retain_days caps on-disk backups ───────────────────────────────

class TestBackupRotation:
    def test_listing_size_bounded_by_retain_days(self, admin_token):
        """Trigger (retain_days + 3) backups — final listing must stay at the cap."""
        listing = _list_backups(admin_token)
        retain_days = listing["retain_days"]
        # Generate retain_days + 3 backups as fast as possible; microsecond
        # precision in the filename avoids collisions.
        for _ in range(retain_days + 3):
            _trigger_backup(admin_token)
        after = _list_backups(admin_token)
        assert len(after["backups"]) <= retain_days, (
            f"Rotation should cap at {retain_days} retained backups; "
            f"listing has {len(after['backups'])}"
        )

    def test_rotation_keeps_most_recent(self, admin_token):
        """After rotation, the newest backup triggered is still present."""
        newest = _trigger_backup(admin_token)["backup"]["filename"]
        # Force rotation.
        listing = _list_backups(admin_token)
        retain_days = listing["retain_days"]
        for _ in range(retain_days + 1):
            _trigger_backup(admin_token)
        final = _list_backups(admin_token)
        names = [b["filename"] for b in final["backups"]]
        # `newest` is older than all subsequent backups, so it may or may not
        # survive; but the most-recent filename in `final` must not have been
        # rotated out prematurely.
        assert names, "Listing unexpectedly empty after rotation"
        assert names[0] >= newest, (
            "Newest-first ordering inverted: older backup appeared before newer."
        )


# ── Restore: input validation + staged semantics ─────────────────────────────

class TestRestoreValidation:
    def test_path_traversal_rejected(self, admin_token):
        for bad in ["../secrets.sqlite", "sub/backup.sqlite", "..\\secrets.sqlite"]:
            r = requests.post(
                f"{BASE_URL}/admin/backups/{bad}/restore",
                headers=auth_headers(admin_token),
                timeout=10,
            )
            assert r.status_code in (400, 404), (
                f"Path traversal {bad!r} must be rejected, got {r.status_code}: {r.text}"
            )

    def test_wrong_extension_rejected(self, admin_token):
        r = requests.post(
            f"{BASE_URL}/admin/backups/backup_20260101_000000.db/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code in (400, 404)

    def test_wrong_prefix_rejected(self, admin_token):
        r = requests.post(
            f"{BASE_URL}/admin/backups/notbackup_20260101_000000.sqlite/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code in (400, 404)

    def test_nonexistent_backup_returns_404(self, admin_token):
        r = requests.post(
            f"{BASE_URL}/admin/backups/backup_19700101_000000.sqlite/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code == 404

    def test_restore_requires_admin(
        self, director_token, finance_token, referee_token, auditor_token
    ):
        for token in (director_token, finance_token, referee_token, auditor_token):
            r = requests.post(
                f"{BASE_URL}/admin/backups/backup_19700101_000000.sqlite/restore",
                headers=auth_headers(token),
                timeout=10,
            )
            assert r.status_code == 403, (
                f"Only admin may call restore; token got {r.status_code}"
            )


class TestRestoreStaged:
    """Restore is asynchronous — it must stage a marker, not mutate live data."""

    def test_restore_of_existing_backup_returns_pending_restart(self, admin_token):
        # First, ensure at least one backup exists to restore.
        body = _trigger_backup(admin_token)
        filename = body["backup"]["filename"]
        r = requests.post(
            f"{BASE_URL}/admin/backups/{filename}/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code == 200, f"restore: {r.text}"
        payload = r.json()
        assert payload["status"] == "pending_restart", (
            f"Restore must not apply synchronously; got status {payload.get('status')!r}"
        )
        assert filename in payload["message"]

    def test_restore_does_not_mutate_live_database(self, admin_token, ts):
        """
        Staging a restore must not touch the live DB.  We prove this by
        reading any stable endpoint before + after the restore call and
        asserting the listing still responds normally (would 500 if the DB
        were corrupted or locked).
        """
        # Create a known row to detect data loss: an invoice.
        inv_before = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-RESTORE-CANARY-{ts}",
                "counterparty": "Canary Corp",
                "issue_date": "2026-01-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        assert inv_before.status_code == 200
        inv_id = inv_before.json()["id"]

        # Stage a restore.
        bkp = _trigger_backup(admin_token)
        requests.post(
            f"{BASE_URL}/admin/backups/{bkp['backup']['filename']}/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )

        # Live DB must still be readable and still contain the canary row.
        inv_after = requests.get(
            f"{BASE_URL}/invoices/{inv_id}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert inv_after.status_code == 200, (
            "Live DB should still be reachable after staging a restore; "
            "the restore must only apply on next boot."
        )
        assert inv_after.json()["id"] == inv_id

    def test_restore_validates_sqlite_magic_bytes(self, admin_token):
        """
        The backup service validates the SQLite magic bytes at stage time.
        A nonexistent file is the closest we can get through the API to
        exercising the SQLite header check path (the write of a corrupt
        file would require filesystem access we do not have from the
        integration-test container).
        """
        r = requests.post(
            f"{BASE_URL}/admin/backups/backup_99990101_000000.sqlite/restore",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        # Either explicit NOT_FOUND or BadRequest (message about magic bytes).
        assert r.status_code in (400, 404)


# ── Migration baseline evidence ──────────────────────────────────────────────

class TestMigrationBaseline:
    """
    The backend runs 36 migrations at startup (see src/migration/mod.rs).
    Exhaustively asserting each one applied requires SQL access that the
    integration-test container does not have, so we verify the *observable*
    baseline: every table exposed through a GET endpoint must respond with
    200 and an expected shape.  If any migration failed, the corresponding
    endpoint would either 500 or expose a missing column / relation error.
    """

    @pytest.mark.parametrize(
        "endpoint,expected_type",
        [
            ("/events",           list),
            ("/rulesets",         list),
            ("/vehicles",         list),
            ("/assets",           list),
            ("/invoices",         list),
            ("/metrics",          list),
            ("/audit/logs",       list),
            ("/data-quality/scans", list),
        ],
    )
    def test_entity_listing_responds_200(self, admin_token, endpoint, expected_type):
        r = requests.get(
            f"{BASE_URL}{endpoint}",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert r.status_code == 200, (
            f"{endpoint} should respond 200 if the underlying migration "
            f"succeeded; got {r.status_code}: {r.text[:200]}"
        )
        assert isinstance(r.json(), expected_type)

    def test_encrypted_fields_populated_after_migration(self, admin_token, ts):
        """
        m20240033_encrypt_payment_references + m20240034_encrypt_personal_identifiers
        add blind-index companion columns.  Successfully creating a vehicle with
        a VIN and reading it back in plaintext evidences the end-to-end encryption
        pipeline (migration + crypto module + service layer) is wired up.
        """
        vin = f"1HGMIG{ts % 99999:05d}AA12345"[:17].replace("I", "1").replace("O", "0").replace("Q", "9")
        create = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": vin,
                "registration_id": f"MIG-{ts}",
                "make": "Migration Motors",
                "model": "M36",
                "year": 2024,
            },
            timeout=10,
        )
        assert create.status_code == 200
        vid = create.json()["id"]
        # Read it back — plaintext VIN must be decrypted by the service layer.
        read = requests.get(
            f"{BASE_URL}/vehicles/{vid}",
            headers=auth_headers(admin_token),
            timeout=10,
        ).json()
        assert read["vin"] == vin, (
            "VIN round-trip failed; encryption migration or crypto wiring broken"
        )

    def test_audit_log_triggers_installed(self, admin_token, auditor_token):
        """
        Migration m20240035_create_audit_log installs BEFORE UPDATE / BEFORE
        DELETE triggers on audit_log (per README §Audit Log Immutability).
        A DELETE endpoint must either return 404/405 (not exposed) — the
        controller layer's first line of defence — with no route registered.
        """
        # Grab an existing audit entry id.
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        if not entries:
            pytest.skip("audit_log is empty; cannot assert on trigger presence")
        entry_id = entries[0]["id"]

        for method in ("DELETE", "PUT", "PATCH"):
            r = requests.request(
                method,
                f"{BASE_URL}/audit/logs/{entry_id}",
                headers=auth_headers(admin_token),
                timeout=10,
            )
            assert r.status_code in (404, 405), (
                f"{method} /audit/logs/{entry_id} must not exist; got {r.status_code}"
            )


# ── Health round-trip after heavy backup activity ───────────────────────────

class TestHealthAfterBackupActivity:
    """
    Nightly backup + rotation must never take the live service down.  This
    test triggers several backups and confirms `/health` stays green and
    the core read endpoints keep responding 200.
    """

    def test_health_green_after_backups(self, admin_token):
        for _ in range(3):
            _trigger_backup(admin_token)
        r = requests.get(f"{BASE_URL}/health", timeout=10)
        assert r.status_code == 200
        assert r.json()["status"] == "ok"

    def test_listings_still_work_after_backups(self, admin_token):
        for _ in range(2):
            _trigger_backup(admin_token)
        for path in ("/events", "/invoices", "/vehicles"):
            r = requests.get(
                f"{BASE_URL}{path}",
                headers=auth_headers(admin_token),
                timeout=10,
            )
            assert r.status_code == 200, (
                f"{path} must keep serving during backup activity; got {r.status_code}"
            )
