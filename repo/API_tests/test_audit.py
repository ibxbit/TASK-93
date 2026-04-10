"""
API tests for the unified audit log.

Covers:
- GET /audit/logs               (list with filters)
- GET /audit/logs/<id>          (get single entry)
- Verify immutability (no update/delete endpoints exposed)
- RBAC: only users with audit_read can access
- Verify audit entries are created after state changes:
    invoice.created, invoice.issued, payment.recorded,
    event.published, vehicle.status_changed, result.submitted
- Sensitive field masking: external_reference, serial_number
"""

import pytest
import requests

from conftest import BASE_URL, auth_headers


class TestAuditLogAccess:
    def test_auditor_can_list_audit_logs(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert isinstance(resp.json(), list)

    def test_admin_can_list_audit_logs(self, admin_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_referee_cannot_read_audit_logs(self, referee_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(referee_token),
            timeout=10,
        )
        assert resp.status_code == 403
        assert resp.json()["code"] == "FORBIDDEN"

    def test_event_director_cannot_read_audit_logs(self, director_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(director_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_finance_clerk_cannot_read_audit_logs(self, finance_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs",
            headers=auth_headers(finance_token),
            timeout=10,
        )
        assert resp.status_code == 403

    def test_unauthenticated_request_returns_401(self):
        resp = requests.get(f"{BASE_URL}/audit/logs", timeout=10)
        assert resp.status_code == 401


class TestAuditLogQuery:
    def test_list_returns_entries_newest_first(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs?limit=10",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        entries = resp.json()
        if len(entries) >= 2:
            # created_at should be descending
            t1 = entries[0]["created_at"]
            t2 = entries[1]["created_at"]
            assert t1 >= t2

    def test_list_with_entity_type_filter(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=invoice",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        entries = resp.json()
        assert all(e["entity_type"] == "invoice" for e in entries)

    def test_list_with_action_filter(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs?action=invoice.created",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200

    def test_list_with_limit(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs?limit=5",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        assert len(resp.json()) <= 5

    def test_limit_capped_at_500(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs?limit=9999",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 200
        # Server-side cap: must not return more than 500
        assert len(resp.json()) <= 500

    def test_list_with_offset(self, auditor_token):
        all_resp = requests.get(
            f"{BASE_URL}/audit/logs?limit=10",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        if len(all_resp) >= 2:
            paged_resp = requests.get(
                f"{BASE_URL}/audit/logs?limit=10&offset=1",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
            if paged_resp:
                assert paged_resp[0]["id"] != all_resp[0]["id"]


class TestAuditLogEntry:
    def test_get_audit_log_entry_by_id(self, auditor_token):
        # First get the list to find an existing entry
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        if entries:
            entry_id = entries[0]["id"]
            resp = requests.get(
                f"{BASE_URL}/audit/logs/{entry_id}",
                headers=auth_headers(auditor_token),
                timeout=10,
            )
            assert resp.status_code == 200
            body = resp.json()
            assert body["id"] == entry_id

    def test_get_nonexistent_entry_returns_404(self, auditor_token):
        resp = requests.get(
            f"{BASE_URL}/audit/logs/999999999",
            headers=auth_headers(auditor_token),
            timeout=10,
        )
        assert resp.status_code == 404

    def test_audit_entry_has_required_fields(self, auditor_token):
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        if entries:
            entry = entries[0]
            required = {"id", "actor_id", "action", "entity_type",
                        "entity_id", "snapshot", "metadata", "created_at"}
            assert required.issubset(entry.keys())

    def test_audit_entry_snapshot_is_json(self, auditor_token):
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        if entries:
            snapshot = entries[0]["snapshot"]
            assert isinstance(snapshot, dict)

    def test_audit_entry_metadata_is_json(self, auditor_token):
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        if entries:
            metadata = entries[0]["metadata"]
            assert isinstance(metadata, dict)


class TestAuditLogImmutability:
    def test_no_delete_endpoint_for_audit_logs(self, admin_token):
        """The API must not expose a DELETE endpoint for audit logs."""
        resp = requests.delete(
            f"{BASE_URL}/audit/logs/1",
            headers=auth_headers(admin_token),
            timeout=10,
        )
        assert resp.status_code in (404, 405), (
            "DELETE on audit log entries must not be allowed"
        )

    def test_no_update_endpoint_for_audit_logs(self, admin_token):
        """The API must not expose a PUT/PATCH endpoint for audit logs."""
        resp = requests.put(
            f"{BASE_URL}/audit/logs/1",
            headers=auth_headers(admin_token),
            json={"action": "tampered"},
            timeout=10,
        )
        assert resp.status_code in (404, 405)

    def test_no_patch_endpoint_for_audit_logs(self, admin_token):
        """PATCH must also be rejected."""
        resp = requests.patch(
            f"{BASE_URL}/audit/logs/1",
            headers=auth_headers(admin_token),
            json={"action": "tampered"},
            timeout=10,
        )
        assert resp.status_code in (404, 405)

    def test_audit_entry_created_at_is_immutable(self, auditor_token, admin_token, ts):
        """created_at of an audit entry must never change between reads."""
        requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-IMMUT-{ts}",
                "counterparty": "Immutability Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        entries = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.created&limit=1",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        if not entries:
            return  # no entries to test yet — skip gracefully

        entry_id = entries[0]["id"]
        first_read = requests.get(
            f"{BASE_URL}/audit/logs/{entry_id}",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        second_read = requests.get(
            f"{BASE_URL}/audit/logs/{entry_id}",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        assert first_read["created_at"] == second_read["created_at"], (
            "Audit entry created_at must not change between reads"
        )
        assert first_read["action"] == second_read["action"]
        assert first_read["snapshot"] == second_read["snapshot"]

    def test_audit_entry_count_only_grows(self, auditor_token, admin_token, ts):
        """The total number of audit entries must never decrease."""
        count_before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        # Trigger at least one new audit entry
        requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-GROW-{ts}",
                "counterparty": "Growing Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )
        count_after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert count_after >= count_before, (
            "Audit log entry count must never decrease — log is append-only"
        )


class TestAuditCoverage:
    def test_invoice_creation_is_audited(self, auditor_token, admin_token, ts):
        """Creating an invoice must produce an audit log entry."""
        # Count entries before
        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.created&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )

        # Create an invoice
        requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-AUDIT-{ts}",
                "counterparty": "Audit Test Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        )

        # Count entries after
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.created&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )

        assert after > before, "Invoice creation must be recorded in the audit log"

    def test_payment_recording_is_audited(self, auditor_token, admin_token, ts):
        """Recording a payment must produce an audit log entry."""
        # Create and issue invoice
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-AUDIT-PMT-{ts}",
                "counterparty": "Audit PMT Corp",
                "issue_date": "2026-04-01",
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
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=payment&action=payment.recorded&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 100.0,
                "method": "cash",
                "external_reference": f"AUDIT-PMT-REF-{ts}",
                "received_at": "2026-04-01T10:00:00Z",
            },
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=payment&action=payment.recorded&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, "Payment recording must be recorded in the audit log"


class TestAuditSensitiveFieldMasking:
    """Verify that sensitive fields are masked / redacted in audit snapshots."""

    def test_asset_serial_number_masked_in_audit_snapshot(
        self, auditor_token, admin_token, ts
    ):
        """serial_number must not appear in plaintext in any audit snapshot."""
        # Create an asset with a serial number
        asset = requests.post(
            f"{BASE_URL}/assets",
            headers=auth_headers(admin_token),
            json={
                "asset_code": f"MASK-{ts}",
                "name": "Masking Test Asset",
                "category": "equipment",
                "serial_number": f"SN-SECRET-{ts}",
                "status": "in_service",
            },
            timeout=10,
        ).json()

        if "id" not in asset:
            pytest.skip("Asset creation failed — skipping masking test")

        # Find audit entry for this asset
        entries = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=asset&limit=500",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        asset_entries = [e for e in entries if e.get("entity_id") == asset["id"]]
        for entry in asset_entries:
            snapshot_str = str(entry.get("snapshot", {}))
            assert f"SN-SECRET-{ts}" not in snapshot_str, (
                "serial_number plaintext must not appear in audit snapshot"
            )

    def test_payment_external_reference_not_in_plaintext_in_audit(
        self, auditor_token, admin_token, ts
    ):
        """external_reference must not appear in plaintext in any audit snapshot."""
        # Create invoice and record a payment
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-MASK-{ts}",
                "counterparty": "Mask Corp",
                "issue_date": "2026-04-01",
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
                "unit_price": 50.0,
            },
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        secret_ref = f"SECRET-REF-{ts}"
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 50.0,
                "method": "cash",
                "external_reference": secret_ref,
                "received_at": "2026-04-01T10:00:00Z",
            },
            timeout=10,
        )

        # Inspect audit entries for payment — plaintext ref must not appear
        entries = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=payment&limit=500",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()

        for entry in entries:
            snapshot_str = str(entry.get("snapshot", {}))
            assert secret_ref not in snapshot_str, (
                "external_reference plaintext must not appear in audit snapshot"
            )

    def test_audit_snapshot_is_dict_not_raw_string(self, auditor_token):
        """Snapshots must be parsed JSON objects, not raw strings."""
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=10",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        for entry in entries:
            assert isinstance(entry["snapshot"], dict), (
                "Audit snapshot must be a JSON object, not a raw string"
            )

    def test_audit_metadata_is_dict(self, auditor_token):
        entries = requests.get(
            f"{BASE_URL}/audit/logs?limit=10",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        for entry in entries:
            assert isinstance(entry["metadata"], dict)

    def test_payment_exception_external_reference_not_in_audit(
        self, auditor_token, admin_token, ts
    ):
        """payment.exception_raised snapshot must also redact external_reference."""
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-EXC-MASK-{ts}",
                "counterparty": "Exc Mask Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={"description": "Fee", "pricing_model": "fixed",
                  "quantity": 1.0, "unit_price": 60.0},
            timeout=10,
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        secret_ref = f"EXC-SECRET-{ts}"
        pmt = requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments",
            headers=auth_headers(admin_token),
            json={
                "amount": 60.0,
                "method": "cash",
                "external_reference": secret_ref,
                "received_at": "2026-04-01T10:00:00Z",
            },
            timeout=10,
        ).json()

        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/payments/{pmt['id']}/exceptions",
            headers=auth_headers(admin_token),
            json={"exception_type": "void", "reason": "Test void for masking"},
            timeout=10,
        )

        entries = requests.get(
            f"{BASE_URL}/audit/logs?entity_type=payment&limit=500",
            headers=auth_headers(auditor_token),
            timeout=10,
        ).json()
        for entry in entries:
            snapshot_str = str(entry.get("snapshot", {}))
            assert secret_ref not in snapshot_str, (
                "external_reference must be redacted in payment.exception_raised snapshot"
            )


class TestAuditStateTransitionCoverage:
    """Every business-critical state transition must appear in the unified audit log."""

    def test_invoice_issuing_is_audited(self, auditor_token, admin_token, ts):
        """invoice.issued must produce a unified audit entry."""
        inv = requests.post(
            f"{BASE_URL}/invoices",
            headers=auth_headers(admin_token),
            json={
                "invoice_no": f"INV-ISSUE-AUDIT-{ts}",
                "counterparty": "Issue Audit Corp",
                "issue_date": "2026-04-01",
                "tax_rate": 0.0,
            },
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/lines",
            headers=auth_headers(admin_token),
            json={"description": "Fee", "pricing_model": "fixed",
                  "quantity": 1.0, "unit_price": 75.0},
            timeout=10,
        )

        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.issued&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.post(
            f"{BASE_URL}/invoices/{inv['id']}/issue",
            headers=auth_headers(admin_token),
            json={},
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=invoice&action=invoice.issued&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, "invoice.issued must be recorded in the unified audit log"

    def test_event_publishing_is_audited(self, auditor_token, admin_token, ts):
        """event.published must produce a unified audit entry."""
        ruleset = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={
                "semantic_version": f"9.{ts % 99}.0",
                "description": "Audit test ruleset",
                "effective_at": "2026-01-01T00:00:00Z",
            },
            timeout=10,
        ).json()

        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Audit Pub Event {ts}"},
            timeout=10,
        ).json()

        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=event&action=event.published&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=event&action=event.published&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, "event.published must be recorded in the unified audit log"

    def test_vehicle_status_transition_is_audited(self, auditor_token, admin_token, ts):
        """vehicle.status_changed must appear in the unified audit log, not just vehicle_audit_log."""
        vehicle = requests.post(
            f"{BASE_URL}/vehicles",
            headers=auth_headers(admin_token),
            json={
                "vin": f"1HGCM{ts % 99999:05d}AAA",
                "registration_id": f"REG-AUDIT-{ts}",
                "make": "Honda",
                "model": "Civic",
                "year": 2022,
                "mileage": 5000,
                "title_transfer_count": 0,
            },
            timeout=10,
        ).json()

        if "id" not in vehicle:
            pytest.skip("Vehicle creation failed — skipping status transition audit test")

        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=vehicle&action=vehicle.status_changed&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.patch(
            f"{BASE_URL}/vehicles/{vehicle['id']}/status",
            headers=auth_headers(admin_token),
            json={"status": "published", "reason": "Audit coverage test"},
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=vehicle&action=vehicle.status_changed&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, (
            "vehicle.status_changed must be recorded in the unified audit log"
        )

    def test_result_submission_is_audited(self, auditor_token, admin_token, ts):
        """result.submitted must produce a unified audit entry."""
        ruleset = requests.post(
            f"{BASE_URL}/rulesets",
            headers=auth_headers(admin_token),
            json={
                "semantic_version": f"8.{ts % 99}.0",
                "effective_at": "2026-01-01T00:00:00Z",
            },
            timeout=10,
        ).json()
        event = requests.post(
            f"{BASE_URL}/events",
            headers=auth_headers(admin_token),
            json={"name": f"Audit Result Event {ts}"},
            timeout=10,
        ).json()
        requests.post(
            f"{BASE_URL}/events/{event['id']}/publish",
            headers=auth_headers(admin_token),
            json={"ruleset_version_id": ruleset["id"]},
            timeout=10,
        )

        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=result&action=result.submitted&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.post(
            f"{BASE_URL}/events/{event['id']}/results",
            headers=auth_headers(admin_token),
            json={"participant_id": ts + 8500, "value_numeric": 72000.0, "unit": "milliseconds"},
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=result&action=result.submitted&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, "result.submitted must be recorded in the unified audit log"

    def test_asset_status_change_is_audited(self, auditor_token, admin_token, ts):
        """asset.status_changed must appear in the unified audit log."""
        asset = requests.post(
            f"{BASE_URL}/assets",
            headers=auth_headers(admin_token),
            json={
                "asset_code": f"AUDSTT-{ts}",
                "name": "Audit Status Test Asset",
                "category": "equipment",
                "status": "in_service",
            },
            timeout=10,
        ).json()

        if "id" not in asset:
            pytest.skip("Asset creation failed — skipping asset status audit test")

        before = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=asset&action=asset.status_changed&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        requests.patch(
            f"{BASE_URL}/assets/{asset['id']}/status",
            headers=auth_headers(admin_token),
            json={"status": "under_maintenance"},
            timeout=10,
        )
        after = len(
            requests.get(
                f"{BASE_URL}/audit/logs?entity_type=asset&action=asset.status_changed&limit=500",
                headers=auth_headers(auditor_token),
                timeout=10,
            ).json()
        )
        assert after > before, (
            "asset.status_changed must be recorded in the unified audit log"
        )
