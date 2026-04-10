"""
Unit tests for field-level AES-256-GCM encryption logic.

Mirrors the algorithm in src/crypto.rs.
No server required — pure Python specification tests.

Covers:
- AES-256-GCM wire format: base64(nonce_12 || ciphertext_with_tag)
- Encrypt/decrypt round-trip correctness
- Fresh random nonce per write (semantic security: same plaintext → different ciphertext)
- Blind-index (keyed FNV-1a hash) stability and uniqueness properties
- Key validation: wrong length, non-base64, empty string
- Masking token: cipher.mask() returns a non-empty redaction string
- Encrypted field layout in a snapshot dict (structured vs raw)
"""

import base64
import hashlib
import hmac
import os
import struct
import pytest

# ── Minimal AES-256-GCM helpers (mirrors Rust aes_gcm behaviour) ─────────────
# We use the `cryptography` package when available; fall back to a
# software-only reference if not installed.  Either way the PROPERTIES
# being tested are the same.

try:
    from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    _HAS_CRYPTO = True
except ImportError:
    _HAS_CRYPTO = False

NONCE_LEN = 12        # 96-bit nonce (AES-GCM standard)
TAG_LEN   = 16        # GCM authentication tag
KEY_LEN   = 32        # AES-256

# Stable test key (never use in production)
_TEST_KEY_B64 = base64.b64encode(b"A" * KEY_LEN).decode()
_TEST_KEY     = b"A" * KEY_LEN


def _validate_key(b64_key: str) -> bytes:
    """Mirror Cipher::from_base64_key: decode and validate length."""
    try:
        raw = base64.b64decode(b64_key)
    except Exception as exc:
        raise ValueError(f"ENCRYPTION_KEY is not valid base64: {exc}") from exc
    if len(raw) != KEY_LEN:
        raise ValueError(
            f"ENCRYPTION_KEY must decode to exactly {KEY_LEN} bytes (AES-256), "
            f"got {len(raw)} bytes"
        )
    return raw


def _encrypt(key: bytes, plaintext: str) -> str:
    """Encrypt plaintext → base64(nonce || ciphertext_with_tag)."""
    nonce = os.urandom(NONCE_LEN)
    pt_bytes = plaintext.encode()          # UTF-8 bytes — may be longer than len(plaintext)
    if _HAS_CRYPTO:
        ct_and_tag = AESGCM(key).encrypt(nonce, pt_bytes, None)
    else:
        # Deterministic XOR-based stub so tests can run without cryptography pkg.
        # Does NOT provide real security; validates structure only.
        ct_and_tag = bytes(b ^ k for b, k in zip(
            pt_bytes,
            (key * (len(pt_bytes) // KEY_LEN + 1))[:len(pt_bytes)]
        )) + b"\x00" * TAG_LEN
    blob = nonce + ct_and_tag
    return base64.b64encode(blob).decode()


def _decrypt(key: bytes, b64_blob: str) -> str:
    """Decrypt base64(nonce || ciphertext_with_tag) → plaintext."""
    blob = base64.b64decode(b64_blob)
    if len(blob) <= NONCE_LEN:
        raise ValueError("Ciphertext too short — nonce missing")
    nonce, ct_and_tag = blob[:NONCE_LEN], blob[NONCE_LEN:]
    if _HAS_CRYPTO:
        return AESGCM(key).decrypt(nonce, ct_and_tag, None).decode()
    # Stub: undo the XOR
    ct = ct_and_tag[:-TAG_LEN]
    return bytes(b ^ k for b, k in zip(
        ct,
        (key * (len(ct) // KEY_LEN + 1))[:len(ct)]
    )).decode()


def _fnv1a_digest(key: bytes, plaintext: str) -> str:
    """
    Keyed FNV-1a blind index (mirrors Cipher::digest in crypto.rs).

    The key is mixed into the FNV offset basis using the first 8 bytes of the
    AES key, making the output opaque without the key.
    """
    FNV_OFFSET = 14_695_981_039_346_656_037
    FNV_PRIME  = 1_099_511_628_211
    MASK64     = 0xFFFF_FFFF_FFFF_FFFF

    key_part = struct.unpack("<Q", key[:8])[0]  # first 8 bytes as little-endian u64
    h = FNV_OFFSET ^ key_part
    for byte in plaintext.encode("utf-8"):
        h ^= byte
        h = (h * FNV_PRIME) & MASK64
    return format(h, "016x")


def mask() -> str:
    """Return the static redaction token (mirrors Cipher::mask in crypto.rs)."""
    return "[REDACTED]"


# ── Key validation tests ──────────────────────────────────────────────────────

class TestKeyValidation:
    def test_valid_32_byte_key_accepted(self):
        key = _validate_key(_TEST_KEY_B64)
        assert len(key) == KEY_LEN

    def test_key_shorter_than_32_bytes_rejected(self):
        short = base64.b64encode(b"short").decode()
        with pytest.raises(ValueError, match="32 bytes"):
            _validate_key(short)

    def test_key_longer_than_32_bytes_rejected(self):
        long_key = base64.b64encode(b"B" * 64).decode()
        with pytest.raises(ValueError, match="32 bytes"):
            _validate_key(long_key)

    def test_non_base64_key_rejected(self):
        with pytest.raises(ValueError, match="base64"):
            _validate_key("not-valid-base64!!!")

    def test_empty_key_rejected(self):
        with pytest.raises(Exception):
            _validate_key("")

    def test_key_decoded_is_bytes(self):
        key = _validate_key(_TEST_KEY_B64)
        assert isinstance(key, bytes)


# ── Encryption wire format tests ─────────────────────────────────────────────

class TestEncryptionWireFormat:
    def test_ciphertext_is_valid_base64(self):
        blob = _encrypt(_TEST_KEY, "hello")
        # Should not raise
        decoded = base64.b64decode(blob)
        assert len(decoded) > 0

    def test_ciphertext_longer_than_nonce_plus_tag(self):
        plaintext = "test-value"
        blob = base64.b64decode(_encrypt(_TEST_KEY, plaintext))
        # nonce(12) + ciphertext(len(plaintext)) + tag(16)
        assert len(blob) >= NONCE_LEN + len(plaintext.encode()) + TAG_LEN

    def test_nonce_is_first_12_bytes(self):
        blob = base64.b64decode(_encrypt(_TEST_KEY, "abc"))
        nonce = blob[:NONCE_LEN]
        assert len(nonce) == NONCE_LEN

    def test_ciphertext_is_not_plaintext(self):
        plaintext = "sensitive-reference-12345"
        blob = _encrypt(_TEST_KEY, plaintext)
        # The raw blob should not contain the plaintext as a substring
        assert plaintext.encode() not in base64.b64decode(blob)

    def test_empty_string_can_be_encrypted(self):
        blob = _encrypt(_TEST_KEY, "")
        # Should produce at least nonce + tag
        decoded = base64.b64decode(blob)
        assert len(decoded) >= NONCE_LEN + TAG_LEN


# ── Round-trip correctness ────────────────────────────────────────────────────

class TestEncryptDecryptRoundtrip:
    def test_decrypt_recovers_original(self):
        for plaintext in ["ACH-REF-001", "1HGCM82633A004352", "SN-1234567890"]:
            ct = _encrypt(_TEST_KEY, plaintext)
            assert _decrypt(_TEST_KEY, ct) == plaintext

    def test_unicode_plaintext_survives_roundtrip(self):
        plaintext = "Straße 42 / München"
        ct = _encrypt(_TEST_KEY, plaintext)
        assert _decrypt(_TEST_KEY, ct) == plaintext

    def test_long_plaintext_survives_roundtrip(self):
        plaintext = "X" * 500
        ct = _encrypt(_TEST_KEY, plaintext)
        assert _decrypt(_TEST_KEY, ct) == plaintext

    def test_wrong_key_cannot_decrypt(self):
        """Decryption with a different key must fail (GCM authentication)."""
        if not _HAS_CRYPTO:
            pytest.skip("Full GCM auth-tag checking requires the cryptography package")
        ct = _encrypt(_TEST_KEY, "secret")
        wrong_key = b"B" * KEY_LEN
        with pytest.raises(Exception):
            _decrypt(wrong_key, ct)

    def test_truncated_blob_raises(self):
        """A ciphertext shorter than NONCE_LEN should raise on decrypt."""
        with pytest.raises((ValueError, Exception)):
            _decrypt(_TEST_KEY, base64.b64encode(b"short").decode())


# ── Semantic security: fresh nonce per write ──────────────────────────────────

class TestRandomNonce:
    def test_same_plaintext_produces_different_ciphertext(self):
        """Each encryption call uses a fresh random nonce → different output."""
        plaintext = "ACH-PAYMENT-REF-99999"
        ct1 = _encrypt(_TEST_KEY, plaintext)
        ct2 = _encrypt(_TEST_KEY, plaintext)
        # Extremely unlikely to collide (1/2^96 probability)
        assert ct1 != ct2, (
            "Identical ciphertexts for the same plaintext indicate nonce reuse"
        )

    def test_nonces_differ_across_encryptions(self):
        blobs = [base64.b64decode(_encrypt(_TEST_KEY, "value")) for _ in range(5)]
        nonces = [b[:NONCE_LEN] for b in blobs]
        # All 5 nonces should be distinct
        assert len(set(nonces)) == 5, "Nonces must not repeat across encryption calls"


# ── Blind index / digest tests ────────────────────────────────────────────────

class TestBlindIndex:
    def test_same_input_produces_same_digest(self):
        """Blind index must be deterministic for the same (key, plaintext) pair."""
        h1 = _fnv1a_digest(_TEST_KEY, "VIN123")
        h2 = _fnv1a_digest(_TEST_KEY, "VIN123")
        assert h1 == h2

    def test_different_inputs_produce_different_digests(self):
        h1 = _fnv1a_digest(_TEST_KEY, "VIN-A")
        h2 = _fnv1a_digest(_TEST_KEY, "VIN-B")
        assert h1 != h2

    def test_different_keys_produce_different_digests(self):
        """The same plaintext with different keys must yield different indexes."""
        key_a = b"A" * KEY_LEN
        key_b = b"B" * KEY_LEN
        h1 = _fnv1a_digest(key_a, "1HGCM82633A004352")
        h2 = _fnv1a_digest(key_b, "1HGCM82633A004352")
        assert h1 != h2

    def test_digest_is_hex_string(self):
        h = _fnv1a_digest(_TEST_KEY, "test")
        assert all(c in "0123456789abcdef" for c in h)

    def test_digest_is_64_bit_hex(self):
        """FNV-1a 64-bit → 16 hex digits."""
        h = _fnv1a_digest(_TEST_KEY, "example")
        assert len(h) == 16

    def test_empty_string_digest_is_stable(self):
        h1 = _fnv1a_digest(_TEST_KEY, "")
        h2 = _fnv1a_digest(_TEST_KEY, "")
        assert h1 == h2


# ── Masking / redaction token tests ──────────────────────────────────────────

class TestMaskingToken:
    def test_mask_is_non_empty_string(self):
        assert isinstance(mask(), str)
        assert len(mask()) > 0

    def test_mask_does_not_contain_sensitive_data(self):
        """The redaction token must not accidentally reveal any key material."""
        token = mask()
        assert _TEST_KEY_B64 not in token
        assert base64.b64encode(_TEST_KEY).decode() not in token

    def test_mask_is_idempotent(self):
        assert mask() == mask()

    def test_masked_snapshot_hides_plaintext(self):
        """Replacing a field with mask() should hide the original value."""
        snapshot = {
            "id": 42,
            "external_reference": "ACH-SECRET-REF-XYZ",
            "amount": 500.0,
        }
        masked = {k: (mask() if k == "external_reference" else v)
                  for k, v in snapshot.items()}

        assert "ACH-SECRET-REF-XYZ" not in str(masked)
        assert masked["amount"] == 500.0  # non-sensitive fields preserved

    def test_all_sensitive_fields_masked_in_payment_snapshot(self):
        """Every designated sensitive field is masked in a payment snapshot."""
        SENSITIVE = {"external_reference"}
        snapshot = {
            "id": 1,
            "invoice_id": 7,
            "amount": 100.0,
            "method": "cash",
            "external_reference": "TXN-PLAIN-TEXT",
            "status": "active",
        }
        masked = {k: (mask() if k in SENSITIVE else v) for k, v in snapshot.items()}
        for field in SENSITIVE:
            assert masked[field] == mask(), f"{field} must be masked"
        # Non-sensitive fields intact
        assert masked["method"] == "cash"
        assert masked["amount"] == 100.0

    def test_all_sensitive_fields_masked_in_vehicle_snapshot(self):
        SENSITIVE = {"vin", "registration_id"}
        snapshot = {
            "id": 5,
            "vin": "1HGCM82633A004352",
            "registration_id": "ABC-1234",
            "make": "Honda",
            "year": 2022,
        }
        masked = {k: (mask() if k in SENSITIVE else v) for k, v in snapshot.items()}
        for field in SENSITIVE:
            assert masked[field] == mask()
        assert masked["make"] == "Honda"

    def test_all_sensitive_fields_masked_in_asset_snapshot(self):
        SENSITIVE = {"serial_number"}
        snapshot = {
            "id": 3,
            "asset_code": "RADAR-01",
            "serial_number": "SN-SECRET-9999",
            "category": "equipment",
        }
        masked = {k: (mask() if k in SENSITIVE else v) for k, v in snapshot.items()}
        assert masked["serial_number"] == mask()
        assert masked["asset_code"] == "RADAR-01"
