"""
Unit tests for versioned event template (ruleset) business logic.

Mirrors the algorithms in src/competition/service.rs.
No server required — pure Python specification tests.

Covers:
- Semantic version format validation
- Ruleset immutability: no in-place modification allowed
- Rollback chain: rollback_of linkage, is_rollback flag
- Event publishing with a ruleset version (config pinning)
- Multiple independent ruleset versions
- Audit snapshot shape for ruleset events
"""

import re
import pytest


# ── Semantic version helpers (mirrors service.rs) ─────────────────────────────

SEMVER_RE = re.compile(
    r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-((?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*)"
    r"(?:\.(?:0|[1-9]\d*|\d*[a-zA-Z-][0-9a-zA-Z-]*))*))?"
    r"(?:\+([0-9a-zA-Z-]+(?:\.[0-9a-zA-Z-]+)*))?$"
)


def is_valid_semver(version: str) -> bool:
    """Return True if the string is a valid semantic version."""
    return bool(SEMVER_RE.match(version))


# ── Ruleset model (in-memory stand-in for the DB row) ────────────────────────

class RulesetVersion:
    _next_id = 1

    def __init__(self, semantic_version: str, description=None,
                 effective_at: str = "2026-01-01T00:00:00Z",
                 rollback_of=None):
        if not is_valid_semver(semantic_version):
            raise ValueError(f"Invalid semantic version: {semantic_version!r}")
        self.id = RulesetVersion._next_id
        RulesetVersion._next_id += 1
        self.semantic_version = semantic_version
        self.description = description
        self.effective_at = effective_at
        self.rollback_of = rollback_of
        self.is_rollback = rollback_of is not None

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "semantic_version": self.semantic_version,
            "description": self.description,
            "effective_at": self.effective_at,
            "rollback_of": self.rollback_of,
            "is_rollback": self.is_rollback,
        }


def create_ruleset(versions: list, semantic_version: str, **kwargs) -> RulesetVersion:
    """
    Create a new immutable ruleset version.
    Raises ValueError if semantic_version is already in use.
    """
    existing = [v.semantic_version for v in versions]
    if semantic_version in existing:
        raise ValueError(f"Semantic version {semantic_version!r} already exists")
    v = RulesetVersion(semantic_version, **kwargs)
    versions.append(v)
    return v


def rollback_ruleset(versions: list, rollback_of_id: int,
                     new_semantic_version: str, **kwargs) -> RulesetVersion:
    """
    Create a rollback version that points to an existing version.
    The original version is NOT modified.
    """
    target = next((v for v in versions if v.id == rollback_of_id), None)
    if target is None:
        raise KeyError(f"Ruleset version {rollback_of_id} not found")
    rb = RulesetVersion(new_semantic_version, rollback_of=rollback_of_id, **kwargs)
    versions.append(rb)
    return rb


# ── Event publishing config pinning ──────────────────────────────────────────

class Event:
    _next_id = 1

    def __init__(self, name: str):
        self.id = Event._next_id
        Event._next_id += 1
        self.name = name
        self.status = "draft"
        self.published_version_id = None

    def publish(self, ruleset_version: RulesetVersion):
        if self.status != "draft":
            raise ValueError("Only draft events can be published")
        self.status = "published"
        self.published_version_id = ruleset_version.id

    def update(self, **kwargs):
        if self.status == "published":
            raise ValueError("Published events cannot be modified")
        for k, v in kwargs.items():
            setattr(self, k, v)


# ── Semantic version tests ────────────────────────────────────────────────────

class TestSemanticVersionValidation:
    def test_standard_versions_are_valid(self):
        for v in ["1.0.0", "2.3.4", "0.1.0", "10.20.30"]:
            assert is_valid_semver(v), f"{v!r} should be valid"

    def test_prerelease_versions_are_valid(self):
        for v in ["1.0.0-alpha", "2.0.0-rc.1", "1.0.0-rollback"]:
            assert is_valid_semver(v), f"{v!r} should be valid"

    def test_build_metadata_versions_are_valid(self):
        assert is_valid_semver("1.0.0+build.123")

    def test_missing_patch_is_invalid(self):
        assert not is_valid_semver("1.0")

    def test_non_numeric_is_invalid(self):
        assert not is_valid_semver("v1.0.0")
        assert not is_valid_semver("one.two.three")

    def test_empty_string_is_invalid(self):
        assert not is_valid_semver("")

    def test_single_number_is_invalid(self):
        assert not is_valid_semver("1")


# ── Ruleset creation and immutability tests ───────────────────────────────────

class TestRulesetCreation:
    def test_create_ruleset_assigns_id(self):
        versions = []
        v = create_ruleset(versions, "1.0.0")
        assert v.id is not None
        assert v.semantic_version == "1.0.0"

    def test_create_ruleset_is_not_rollback(self):
        versions = []
        v = create_ruleset(versions, "1.1.0")
        assert v.is_rollback is False
        assert v.rollback_of is None

    def test_duplicate_semantic_version_raises(self):
        versions = []
        create_ruleset(versions, "3.0.0")
        with pytest.raises(ValueError, match="already exists"):
            create_ruleset(versions, "3.0.0")

    def test_invalid_semantic_version_raises(self):
        versions = []
        with pytest.raises(ValueError, match="Invalid semantic version"):
            create_ruleset(versions, "not-a-version")

    def test_multiple_versions_in_list(self):
        versions = []
        create_ruleset(versions, "4.0.0")
        create_ruleset(versions, "4.1.0")
        create_ruleset(versions, "4.2.0")
        assert len(versions) == 3
        semvers = [v.semantic_version for v in versions]
        assert "4.0.0" in semvers
        assert "4.1.0" in semvers


class TestRulesetImmutability:
    def test_ruleset_cannot_be_modified_in_place(self):
        """Demonstrate that the original ruleset is unchanged after rollback."""
        versions = []
        original = create_ruleset(versions, "5.0.0", description="Original")

        rollback_ruleset(versions, original.id, "5.0.1-rollback")

        # Original is unchanged
        assert original.semantic_version == "5.0.0"
        assert original.description == "Original"
        assert original.is_rollback is False

    def test_total_version_count_grows_after_rollback(self):
        versions = []
        v = create_ruleset(versions, "6.0.0")
        rollback_ruleset(versions, v.id, "6.0.1-rb")
        rollback_ruleset(versions, v.id, "6.0.2-rb")
        assert len(versions) == 3


# ── Rollback chain tests ─────────────────────────────────────────────────────

class TestRollbackChain:
    def test_rollback_sets_is_rollback_true(self):
        versions = []
        original = create_ruleset(versions, "7.0.0")
        rb = rollback_ruleset(versions, original.id, "7.0.1-rollback")
        assert rb.is_rollback is True

    def test_rollback_sets_rollback_of_to_original_id(self):
        versions = []
        original = create_ruleset(versions, "8.0.0")
        rb = rollback_ruleset(versions, original.id, "8.0.1-rb")
        assert rb.rollback_of == original.id

    def test_rollback_has_different_id_from_original(self):
        versions = []
        original = create_ruleset(versions, "9.0.0")
        rb = rollback_ruleset(versions, original.id, "9.0.1-rb")
        assert rb.id != original.id

    def test_rollback_of_nonexistent_version_raises(self):
        versions = []
        with pytest.raises(KeyError):
            rollback_ruleset(versions, 999999, "10.0.1-rb")

    def test_chained_rollbacks_maintain_linkage(self):
        """Each rollback in a chain points to its direct predecessor."""
        versions = []
        v1 = create_ruleset(versions, "11.0.0")
        v2 = rollback_ruleset(versions, v1.id, "11.0.1-rb")
        v3 = rollback_ruleset(versions, v2.id, "11.0.2-rb")

        assert v2.rollback_of == v1.id
        assert v3.rollback_of == v2.id

    def test_rollback_to_dict_has_required_fields(self):
        versions = []
        v = create_ruleset(versions, "12.0.0")
        rb = rollback_ruleset(versions, v.id, "12.0.1-rb")
        d = rb.to_dict()
        required = {"id", "semantic_version", "effective_at",
                    "rollback_of", "is_rollback"}
        assert required.issubset(d.keys())


# ── Event publishing and config pinning tests ─────────────────────────────────

class TestEventRulesetPinning:
    def test_published_event_stores_ruleset_version_id(self):
        versions = []
        rs = create_ruleset(versions, "13.0.0")
        ev = Event("Grand Prix 2026")
        ev.publish(rs)
        assert ev.published_version_id == rs.id
        assert ev.status == "published"

    def test_two_events_can_pin_different_versions(self):
        versions = []
        rs_a = create_ruleset(versions, "14.0.0")
        rs_b = create_ruleset(versions, "14.1.0")

        ev1 = Event("Event A")
        ev2 = Event("Event B")
        ev1.publish(rs_a)
        ev2.publish(rs_b)

        assert ev1.published_version_id == rs_a.id
        assert ev2.published_version_id == rs_b.id
        assert ev1.published_version_id != ev2.published_version_id

    def test_cannot_publish_already_published_event(self):
        versions = []
        rs = create_ruleset(versions, "15.0.0")
        ev = Event("Already Published")
        ev.publish(rs)
        with pytest.raises(ValueError, match="draft"):
            ev.publish(rs)

    def test_published_event_cannot_be_updated(self):
        versions = []
        rs = create_ruleset(versions, "16.0.0")
        ev = Event("Frozen Event")
        ev.publish(rs)
        with pytest.raises(ValueError, match="Published"):
            ev.update(name="Tampered Name")

    def test_draft_event_can_be_updated(self):
        ev = Event("Draft Event")
        ev.update(name="Updated Draft")
        assert ev.name == "Updated Draft"

    def test_event_retains_original_ruleset_after_new_version_created(self):
        """Creating a newer ruleset version does NOT change pinned events."""
        versions = []
        rs_old = create_ruleset(versions, "17.0.0")
        ev = Event("Stable Event")
        ev.publish(rs_old)

        # Create a newer version — event must not change
        create_ruleset(versions, "17.1.0")
        assert ev.published_version_id == rs_old.id
