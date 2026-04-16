"""
Unit tests for Role-Based Access Control (RBAC) logic.

Tests verify the authoritative role → permission mapping as specified in
src/rbac/mod.rs.  No server required — pure specification tests.

Covers:
- Per-role permission membership
- Least-privilege: roles cannot escalate beyond their defined permissions
- Full endpoint-to-permission mapping
- No two roles share identical permission sets
- Administrator is a superset of every other role
"""

import pytest

# ── Role and permission definitions (mirrors src/rbac/mod.rs) ─────────────────

ROLES = ["administrator", "event_director", "referee", "finance_clerk", "auditor"]

PERMISSIONS = {
    "system_admin",
    "users_read",
    "users_manage",
    "roles_manage",
    "events_read",
    "events_write",
    "events_delete",
    "participants_read",
    "participants_write",
    "referees_read",
    "referees_write",
    "financials_read",
    "financials_write",
    "audit_read",
}

ROLE_PERMISSIONS = {
    "administrator": {
        "system_admin", "users_read", "users_manage", "roles_manage",
        "events_read", "events_write", "events_delete",
        "participants_read", "participants_write",
        "referees_read", "referees_write",
        "financials_read", "financials_write",
        "audit_read",
    },
    "event_director": {
        "events_read", "events_write", "events_delete",
        "participants_read", "participants_write",
        "referees_read", "referees_write",
    },
    "referee": {
        "events_read", "participants_read", "referees_read",
    },
    "finance_clerk": {
        "events_read", "financials_read", "financials_write",
    },
    "auditor": {
        "users_read", "events_read", "participants_read",
        "financials_read", "audit_read",
    },
}


# ── Tests ─────────────────────────────────────────────────────────────────────

class TestAdministratorRole:
    def test_administrator_has_all_permissions(self):
        admin_perms = ROLE_PERMISSIONS["administrator"]
        assert admin_perms == PERMISSIONS, "Administrator must hold every permission"

    def test_administrator_has_system_admin(self):
        assert "system_admin" in ROLE_PERMISSIONS["administrator"]

    def test_administrator_has_audit_read(self):
        assert "audit_read" in ROLE_PERMISSIONS["administrator"]

    def test_administrator_has_roles_manage(self):
        assert "roles_manage" in ROLE_PERMISSIONS["administrator"]

    def test_administrator_has_financials_write(self):
        assert "financials_write" in ROLE_PERMISSIONS["administrator"]


class TestEventDirectorRole:
    def test_event_director_has_event_write(self):
        assert "events_write" in ROLE_PERMISSIONS["event_director"]

    def test_event_director_has_no_financials(self):
        assert "financials_read" not in ROLE_PERMISSIONS["event_director"]
        assert "financials_write" not in ROLE_PERMISSIONS["event_director"]

    def test_event_director_has_no_audit_read(self):
        assert "audit_read" not in ROLE_PERMISSIONS["event_director"]

    def test_event_director_has_no_system_admin(self):
        assert "system_admin" not in ROLE_PERMISSIONS["event_director"]

    def test_event_director_can_manage_participants(self):
        assert "participants_read" in ROLE_PERMISSIONS["event_director"]
        assert "participants_write" in ROLE_PERMISSIONS["event_director"]


class TestRefereeRole:
    def test_referee_is_read_only(self):
        perms = ROLE_PERMISSIONS["referee"]
        write_perms = {p for p in perms if "write" in p or "manage" in p or p == "system_admin"}
        assert write_perms == set(), f"Referee must have no write/manage perms, got: {write_perms}"

    def test_referee_has_events_read(self):
        assert "events_read" in ROLE_PERMISSIONS["referee"]

    def test_referee_has_no_financials(self):
        assert "financials_read" not in ROLE_PERMISSIONS["referee"]
        assert "financials_write" not in ROLE_PERMISSIONS["referee"]

    def test_referee_has_no_audit_read(self):
        assert "audit_read" not in ROLE_PERMISSIONS["referee"]


class TestFinanceClerkRole:
    def test_finance_clerk_has_financials_write(self):
        assert "financials_write" in ROLE_PERMISSIONS["finance_clerk"]

    def test_finance_clerk_has_events_read_for_context(self):
        assert "events_read" in ROLE_PERMISSIONS["finance_clerk"]

    def test_finance_clerk_cannot_write_events(self):
        assert "events_write" not in ROLE_PERMISSIONS["finance_clerk"]

    def test_finance_clerk_cannot_manage_participants(self):
        assert "participants_write" not in ROLE_PERMISSIONS["finance_clerk"]

    def test_finance_clerk_has_no_audit_read(self):
        assert "audit_read" not in ROLE_PERMISSIONS["finance_clerk"]


class TestAuditorRole:
    def test_auditor_is_read_only(self):
        perms = ROLE_PERMISSIONS["auditor"]
        write_perms = {p for p in perms if "write" in p or "manage" in p or p == "system_admin"}
        assert write_perms == set(), f"Auditor must have no write/manage perms, got: {write_perms}"

    def test_auditor_has_audit_read(self):
        assert "audit_read" in ROLE_PERMISSIONS["auditor"]

    def test_auditor_has_financials_read(self):
        assert "financials_read" in ROLE_PERMISSIONS["auditor"]

    def test_auditor_cannot_write_financials(self):
        assert "financials_write" not in ROLE_PERMISSIONS["auditor"]


class TestRoleHierarchy:
    def test_all_roles_have_events_read(self):
        """Every role should at minimum read events for operational awareness."""
        for role in ROLES:
            assert "events_read" in ROLE_PERMISSIONS[role], (
                f"{role} must have events_read"
            )

    def test_only_administrator_has_system_admin(self):
        for role in ROLES:
            if role == "administrator":
                assert "system_admin" in ROLE_PERMISSIONS[role]
            else:
                assert "system_admin" not in ROLE_PERMISSIONS[role], (
                    f"{role} must not have system_admin"
                )

    def test_only_administrator_and_auditor_have_audit_read(self):
        for role in ROLES:
            if role in ("administrator", "auditor"):
                assert "audit_read" in ROLE_PERMISSIONS[role]
            else:
                assert "audit_read" not in ROLE_PERMISSIONS[role], (
                    f"{role} must not have audit_read"
                )

    def test_roles_are_distinct(self):
        """No two roles should have identical permission sets."""
        role_list = list(ROLES)
        for i in range(len(role_list)):
            for j in range(i + 1, len(role_list)):
                r1, r2 = role_list[i], role_list[j]
                assert ROLE_PERMISSIONS[r1] != ROLE_PERMISSIONS[r2], (
                    f"{r1} and {r2} have identical permissions — roles must be distinct"
                )

    def test_administrator_is_superset_of_all(self):
        """Administrator permissions must be a superset of every other role."""
        admin_perms = ROLE_PERMISSIONS["administrator"]
        for role in ROLES:
            if role == "administrator":
                continue
            role_perms = ROLE_PERMISSIONS[role]
            assert role_perms.issubset(admin_perms), (
                f"administrator must include all {role} permissions; "
                f"missing: {role_perms - admin_perms}"
            )


# ── Endpoint → required permission mapping ────────────────────────────────────
#
# This table is the authoritative specification of which permission each
# endpoint requires.  It mirrors the guard annotations in src/*/handlers.rs.
# Tests below assert that at least the correct roles can/cannot call each endpoint.

ENDPOINT_PERMISSIONS = {
    # Events & rulesets
    "POST /events":                             "events_write",
    "GET /events":                              "events_read",
    "GET /events/<id>":                         "events_read",
    "PUT /events/<id>":                         "events_write",
    "POST /events/<id>/publish":                "events_write",
    "POST /rulesets":                           "events_write",
    "GET /rulesets":                            "events_read",
    "GET /rulesets/<id>":                       "events_read",
    "POST /rulesets/<id>/rollback":             "events_write",
    # Results
    "POST /events/<eid>/results":               "participants_write",
    "GET /events/<eid>/rankings":               "events_read",
    "GET /events/<eid>/results/export":         "audit_read",
    "POST /events/<eid>/results/<rid>/reviews": "referees_write",
    "GET /events/<eid>/results/<rid>/reviews":  "events_read",
    "POST /events/<eid>/results/<rid>/arbitrate": "events_write",
    "POST /events/<eid>/results/<rid>/corrections": "participants_write",
    "GET /events/<eid>/results/<rid>/corrections": "events_read",
    "POST /events/<eid>/results/<rid>/corrections/<cid>/resolve": "events_write",
    # Vehicles
    "POST /vehicles":                           "events_write",
    "GET /vehicles":                            "events_read",
    "GET /vehicles/<id>":                       "events_read",
    "PUT /vehicles/<id>":                       "events_write",
    "POST /vehicles/<id>/status":               "events_write",
    "GET /vehicles/<id>/history":               "events_read",
    # Assets
    "POST /assets":                             "events_write",
    "GET /assets":                              "events_read",
    "GET /assets/<id>":                         "events_read",
    "PUT /assets/<id>":                         "events_write",
    "PATCH /assets/<id>/status":                "events_write",
    "GET /assets/<id>/history":                 "events_read",
    "GET /assets/export":                       "events_read",
    "POST /assets/import":                      "events_write",
    # Billing
    "POST /invoices":                           "financials_write",
    "GET /invoices":                            "financials_read",
    "GET /invoices/<id>":                       "financials_read",
    "POST /invoices/<id>/lines":                "financials_write",
    "POST /invoices/<id>/issue":                "financials_write",
    "POST /invoices/<iid>/payments":            "financials_write",
    "GET /invoices/<iid>/payments":             "financials_read",
    "POST /invoices/<iid>/payments/<pid>/exceptions": "financials_write",
    "GET /invoices/<iid>/payments/<pid>/exceptions":  "financials_read",
    "POST /invoices/<iid>/payments/<pid>/refunds":    "financials_write",
    # Audit logs
    "GET /audit/logs":                          "audit_read",
    "GET /audit/logs/<id>":                     "audit_read",
    # Analytics & data quality
    "POST /metrics":                            "financials_write",
    "GET /metrics":                             "financials_read",
    "PUT /metrics/<id>":                        "financials_write",
    "GET /analytics/trends":                    "audit_read",
    "GET /analytics/funnel":                    "audit_read",
    "GET /analytics/retention":                 "audit_read",
    "GET /analytics/export":                    "audit_read",
    "POST /data-quality/scans":                 "audit_read",
    "GET /data-quality/scans":                  "audit_read",
    "GET /data-quality/scans/<id>":             "audit_read",
    # Administration
    "POST /admin/roles/assign":                 "roles_manage",
    "POST /admin/roles/revoke":                 "roles_manage",
    "GET /admin/users/<id>/roles":              "roles_manage",
    "GET /admin/backups":                       "system_admin",
    "POST /admin/backups":                      "system_admin",
    "POST /admin/backups/<f>/restore":          "system_admin",
}


def has_permission(role: str, permission: str) -> bool:
    """Return True if the given role holds the given permission."""
    return permission in ROLE_PERMISSIONS.get(role, set())


def can_call(role: str, endpoint: str) -> bool:
    """Return True if the role has the permission required by the endpoint."""
    required = ENDPOINT_PERMISSIONS.get(endpoint)
    if required is None:
        raise KeyError(f"Endpoint not in mapping: {endpoint!r}")
    return has_permission(role, required)


class TestEndpointPermissionMapping:
    """Each endpoint must map to exactly one permission; the table must be complete."""

    def test_all_endpoints_have_exactly_one_permission(self):
        for endpoint, perm in ENDPOINT_PERMISSIONS.items():
            assert perm in PERMISSIONS, (
                f"Endpoint {endpoint!r} references unknown permission {perm!r}"
            )

    def test_admin_can_call_all_endpoints(self):
        for endpoint in ENDPOINT_PERMISSIONS:
            assert can_call("administrator", endpoint), (
                f"administrator must be able to call {endpoint!r}"
            )

    def test_referee_read_only_endpoints(self):
        readable = {e for e, p in ENDPOINT_PERMISSIONS.items()
                    if p in ROLE_PERMISSIONS["referee"]}
        for endpoint in readable:
            assert can_call("referee", endpoint)

    def test_referee_cannot_call_write_endpoints(self):
        write_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if "write" in p or p in ("system_admin", "roles_manage")]
        for endpoint in write_endpoints:
            assert not can_call("referee", endpoint), (
                f"referee must not be able to call write endpoint {endpoint!r}"
            )

    def test_auditor_can_access_all_audit_endpoints(self):
        audit_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if p == "audit_read"]
        for endpoint in audit_endpoints:
            assert can_call("auditor", endpoint), (
                f"auditor must be able to call audit endpoint {endpoint!r}"
            )

    def test_auditor_cannot_call_any_write_endpoint(self):
        write_perms = {p for p in PERMISSIONS if "write" in p or p == "system_admin"}
        write_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if p in write_perms]
        for endpoint in write_endpoints:
            assert not can_call("auditor", endpoint), (
                f"auditor must not call write endpoint {endpoint!r}"
            )

    def test_finance_clerk_can_access_billing_endpoints(self):
        billing = [e for e, p in ENDPOINT_PERMISSIONS.items()
                   if p in ("financials_read", "financials_write")]
        for endpoint in billing:
            assert can_call("finance_clerk", endpoint)

    def test_finance_clerk_cannot_access_audit_endpoints(self):
        audit_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if p == "audit_read"]
        for endpoint in audit_endpoints:
            assert not can_call("finance_clerk", endpoint)

    def test_finance_clerk_cannot_access_admin_endpoints(self):
        admin_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if p in ("system_admin", "roles_manage")]
        for endpoint in admin_endpoints:
            assert not can_call("finance_clerk", endpoint)

    def test_event_director_cannot_access_billing(self):
        billing = [e for e, p in ENDPOINT_PERMISSIONS.items()
                   if p in ("financials_read", "financials_write")]
        for endpoint in billing:
            assert not can_call("event_director", endpoint)

    def test_event_director_cannot_access_audit_endpoints(self):
        audit_endpoints = [e for e, p in ENDPOINT_PERMISSIONS.items()
                           if p == "audit_read"]
        for endpoint in audit_endpoints:
            assert not can_call("event_director", endpoint)


class TestLeastPrivilege:
    """Every role must have the minimum permissions needed — no excess grants."""

    def test_referee_only_has_read_permissions(self):
        perms = ROLE_PERMISSIONS["referee"]
        assert all("read" in p or p == "events_read" or "read" in p for p in perms)

    def test_finance_clerk_does_not_have_events_write(self):
        assert "events_write" not in ROLE_PERMISSIONS["finance_clerk"]

    def test_finance_clerk_does_not_have_participants_write(self):
        assert "participants_write" not in ROLE_PERMISSIONS["finance_clerk"]

    def test_auditor_does_not_have_financials_write(self):
        assert "financials_write" not in ROLE_PERMISSIONS["auditor"]

    def test_event_director_does_not_have_financials_read(self):
        assert "financials_read" not in ROLE_PERMISSIONS["event_director"]

    def test_no_role_except_admin_has_system_admin(self):
        for role in ROLES:
            if role != "administrator":
                assert "system_admin" not in ROLE_PERMISSIONS[role]

    def test_no_role_except_admin_and_auditor_has_audit_read(self):
        privileged = {"administrator", "auditor"}
        for role in ROLES:
            if role not in privileged:
                assert "audit_read" not in ROLE_PERMISSIONS[role], (
                    f"{role} must not have audit_read"
                )

    def test_total_permission_count_per_role_is_bounded(self):
        """Sanity: no non-admin role should hold all 14 permissions."""
        for role in ROLES:
            if role != "administrator":
                assert len(ROLE_PERMISSIONS[role]) < len(PERMISSIONS), (
                    f"{role} must not hold all permissions"
                )
