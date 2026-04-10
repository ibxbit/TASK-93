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
        let mut mac = HmacSha256::new_from_slice(&self.key_bytes)
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
