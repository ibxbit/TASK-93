use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use hmac::{Hmac, Mac};
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;
use std::sync::Arc;

const NONCE_LEN: usize = 12; // AES-GCM standard 96-bit nonce

/// AES-256-GCM cipher used for field-level encryption of sensitive data at rest.
///
/// # Encrypted fields
/// - `payment_entries.external_reference` — ACH/cheque/bank references
/// - `vehicles.vin` — ISO 3779 vehicle identifier
/// - `vehicles.registration_id` — licence plate / registration number
/// - `assets.serial_number` — asset serial number (when present)
///
/// # Wire format
/// Encrypted blobs are stored as base64(`nonce_12_bytes || ciphertext_with_tag`).
/// Each call to `encrypt` generates a fresh random 96-bit nonce, so the same
/// plaintext produces a different ciphertext on every write.
///
/// # Blind index
/// Because random nonces break SQL equality lookups, columns that require
/// unique-constraint checking (VIN, external_reference) have a companion
/// `*_hash` column populated via `Cipher::digest`.  The digest is a
/// deterministic keyed hash derived from the 256-bit AES key — opaque without
/// the key, but stable for the same plaintext.
#[derive(Clone)]
pub struct Cipher {
    pub(crate) inner: Arc<Aes256Gcm>,
    /// Raw key bytes — retained for keyed digest derivation.
    key_bytes: [u8; 32],
}

impl Cipher {
    /// Initialise from a **base64-encoded 32-byte** (256-bit) key loaded from
    /// the `ENCRYPTION_KEY` environment variable.
    ///
    /// Returns `Err` if the key is absent, not valid base64, or not exactly
    /// 32 bytes — the application refuses to start on any key error.
    pub fn from_base64_key(b64_key: &str) -> Result<Self, String> {
        let decoded = STANDARD
            .decode(b64_key)
            .map_err(|e| format!("ENCRYPTION_KEY is not valid base64: {e}"))?;

        if decoded.len() != 32 {
            return Err(format!(
                "ENCRYPTION_KEY must decode to exactly 32 bytes (AES-256), \
                 got {} bytes",
                decoded.len()
            ));
        }

        let mut key_bytes = [0u8; 32];
        key_bytes.copy_from_slice(&decoded);

        let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
        Ok(Self {
            inner: Arc::new(Aes256Gcm::new(key)),
            key_bytes,
        })
    }

    // ── Encrypt / decrypt ─────────────────────────────────────────────────────

    /// Encrypt `plaintext` with AES-256-GCM using a fresh random nonce.
    ///
    /// Returns `base64(nonce || ciphertext_with_tag)`.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, String> {
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

        let ciphertext = self
            .inner
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|e| format!("AES-GCM encryption failed: {e}"))?;

        let mut combined = Vec::with_capacity(NONCE_LEN + ciphertext.len());
        combined.extend_from_slice(&nonce);
        combined.extend_from_slice(&ciphertext);

        Ok(STANDARD.encode(&combined))
    }

    /// Decrypt a blob produced by `encrypt`.
    ///
    /// Expects `base64(nonce_12_bytes || ciphertext_with_tag)`.
    pub fn decrypt(&self, b64: &str) -> Result<String, String> {
        let combined = STANDARD
            .decode(b64)
            .map_err(|e| format!("base64 decode error: {e}"))?;

        if combined.len() < NONCE_LEN {
            return Err(format!(
                "Ciphertext too short ({} bytes); expected at least {} bytes",
                combined.len(),
                NONCE_LEN
            ));
        }

        let nonce = Nonce::from_slice(&combined[..NONCE_LEN]);
        let plaintext_bytes = self
            .inner
            .decrypt(nonce, &combined[NONCE_LEN..])
            .map_err(|e| format!("AES-GCM decryption failed: {e}"))?;

        String::from_utf8(plaintext_bytes)
            .map_err(|e| format!("UTF-8 decode error after decryption: {e}"))
    }

    /// Encrypt `value` if `Some`, leaving `None` as-is.
    pub fn encrypt_opt(&self, value: Option<&str>) -> Result<Option<String>, String> {
        match value {
            None => Ok(None),
            Some(v) => self.encrypt(v).map(Some),
        }
    }

    /// Decrypt `value` if `Some`, leaving `None` as-is.
    pub fn decrypt_opt(&self, value: Option<&str>) -> Result<Option<String>, String> {
        match value {
            None => Ok(None),
            Some(v) => self.decrypt(v).map(Some),
        }
    }

    // ── Blind index ───────────────────────────────────────────────────────────

    /// Compute a stable, opaque blind-index digest of `plaintext` keyed by this
    /// cipher's key material.
    ///
    /// Used as a blind index: SQL equality lookups on the `*_hash` companion
    /// column replace lookups on the (now encrypted) primary column.
    ///
    /// Algorithm: HMAC-SHA256 keyed with the 256-bit AES key, truncated to the
    /// first 16 bytes (128 bits) and hex-encoded.  This provides cryptographic
    /// collision resistance and preimage resistance across different key
    /// deployments.
    ///
    /// # Migration note
    /// Changing from the previous FNV-1a algorithm alters all digest outputs.
    /// TODO: existing `*_hash` column values must be recomputed when this change
    /// is first deployed against a populated database.
    pub fn digest(&self, plaintext: &str) -> String {
        let mut mac = <HmacSha256 as KeyInit>::new_from_slice(&self.key_bytes)
            .expect("HMAC accepts any key length");
        mac.update(plaintext.as_bytes());
        let result = mac.finalize().into_bytes();
        // First 16 bytes (128 bits) — compact but collision-resistant index.
        hex::encode(&result[..16])
    }

    // ── Masking (for audit snapshots and exports) ──────────────────────────────

    /// Return a redaction token for use in audit log snapshots and exports.
    ///
    /// Sensitive fields are replaced with this marker so that audit trails
    /// remain readable while PII/PCI data is never written in plaintext.
    pub fn mask() -> &'static str {
        "[REDACTED]"
    }
}

// ══════════════════════════════════════════════════════════════════════════════
// Native Rust unit tests
// ══════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use base64::{engine::general_purpose::STANDARD, Engine};

    /// Build a valid 32-byte base64 key for testing.
    fn test_key() -> String {
        STANDARD.encode([0xAA_u8; 32])
    }

    // ── Key initialisation ──────────────────────────────────────────────────

    #[test]
    fn from_base64_key_accepts_valid_32_byte_key() {
        let cipher = Cipher::from_base64_key(&test_key());
        assert!(cipher.is_ok());
    }

    #[test]
    fn from_base64_key_rejects_short_key() {
        let short = STANDARD.encode([0xBB_u8; 16]); // only 16 bytes
        let result = Cipher::from_base64_key(&short);
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("32 bytes"), "expected '32 bytes' in error: {err}");
    }

    #[test]
    fn from_base64_key_rejects_invalid_base64() {
        let result = Cipher::from_base64_key("!!!not-base64!!!");
        assert!(result.is_err());
        let err = result.err().unwrap();
        assert!(err.contains("base64"), "expected 'base64' in error: {err}");
    }

    // ── Encrypt / decrypt round-trip ────────────────────────────────────────

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let plaintext = "1HGCM82633A004352";
        let ciphertext = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn encrypt_produces_different_ciphertext_each_call() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let ct1 = cipher.encrypt("same-input").unwrap();
        let ct2 = cipher.encrypt("same-input").unwrap();
        // Random nonce guarantees distinct ciphertext for same plaintext.
        assert_ne!(ct1, ct2);
    }

    #[test]
    fn decrypt_rejects_tampered_ciphertext() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let ct = cipher.encrypt("secret").unwrap();
        let mut bytes = STANDARD.decode(&ct).unwrap();
        // Flip a byte in the ciphertext portion (past the nonce).
        if bytes.len() > NONCE_LEN {
            bytes[NONCE_LEN] ^= 0xFF;
        }
        let tampered = STANDARD.encode(&bytes);
        assert!(cipher.decrypt(&tampered).is_err());
    }

    #[test]
    fn decrypt_rejects_too_short_input() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let short = STANDARD.encode([0u8; 5]); // < 12 bytes
        assert!(cipher.decrypt(&short).is_err());
    }

    // ── encrypt_opt / decrypt_opt ───────────────────────────────────────────

    #[test]
    fn encrypt_opt_none_stays_none() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        assert_eq!(cipher.encrypt_opt(None).unwrap(), None);
    }

    #[test]
    fn decrypt_opt_none_stays_none() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        assert_eq!(cipher.decrypt_opt(None).unwrap(), None);
    }

    #[test]
    fn encrypt_decrypt_opt_roundtrip() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let ct = cipher.encrypt_opt(Some("hello")).unwrap();
        assert!(ct.is_some());
        let pt = cipher.decrypt_opt(ct.as_deref()).unwrap();
        assert_eq!(pt, Some("hello".to_owned()));
    }

    // ── Blind index (digest) ────────────────────────────────────────────────

    #[test]
    fn digest_is_deterministic() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let d1 = cipher.digest("1HGCM82633A004352");
        let d2 = cipher.digest("1HGCM82633A004352");
        assert_eq!(d1, d2);
    }

    #[test]
    fn digest_differs_for_different_inputs() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let d1 = cipher.digest("VIN-AAA");
        let d2 = cipher.digest("VIN-BBB");
        assert_ne!(d1, d2);
    }

    #[test]
    fn digest_differs_for_different_keys() {
        let c1 = Cipher::from_base64_key(&STANDARD.encode([0xAA_u8; 32])).unwrap();
        let c2 = Cipher::from_base64_key(&STANDARD.encode([0xBB_u8; 32])).unwrap();
        assert_ne!(c1.digest("same"), c2.digest("same"));
    }

    #[test]
    fn digest_is_32_char_hex() {
        let cipher = Cipher::from_base64_key(&test_key()).unwrap();
        let d = cipher.digest("test");
        assert_eq!(d.len(), 32); // 16 bytes → 32 hex chars
        assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── mask ────────────────────────────────────────────────────────────────

    #[test]
    fn mask_returns_redacted_token() {
        assert_eq!(Cipher::mask(), "[REDACTED]");
    }
}
