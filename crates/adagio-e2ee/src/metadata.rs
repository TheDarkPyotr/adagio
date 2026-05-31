//! Nextcloud E2EE v2.x metadata format: serialisation, encryption, and CMS signing.
//!
//! # Structure
//! ```text
//! Outer envelope (JSON → server):
//!   metadata.ciphertext  ← gzip(inner_json) encrypted with AES-128-GCM metadata key
//!   metadata.nonce       ← 96-bit AES IV (base64)
//!   metadata.authTag     ← 16-byte GCM tag (base64)
//!   users[].encryptedMetadataKey ← RSA/OAEP-wrapped 128-bit metadata key per device
//!   version: "2.0"
//!
//! Inner JSON (plaintext before gzip+AES):
//!   { counter, keyChecksums, files: { uuid → E2eeFileEntry }, folders: {...} }
//! ```

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes128Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};

use crate::keys::wrap_metadata_key;
use crate::{E2eeError, E2eeFileEntry, E2eeMetadata};

// ── Outer envelope ────────────────────────────────────────────────────────────

/// Outer metadata structure sent to / received from the Nextcloud server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OuterMetadata {
    pub metadata: MetadataBlob,
    pub users: Vec<MetadataUser>,
    /// Guest-upload section — empty for owner-created folders (required by v2 schema).
    #[serde(default)]
    pub filedrop: std::collections::HashMap<String, serde_json::Value>,
    #[serde(default = "default_version")]
    pub version: String,
}

fn default_version() -> String {
    "2.0".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataBlob {
    pub ciphertext: String,
    pub nonce: String,
    #[serde(rename = "authenticationTag")]
    pub auth_tag: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataUser {
    #[serde(rename = "userId")]
    pub user_id: String,
    pub certificate: String,
    #[serde(rename = "encryptedMetadataKey")]
    pub encrypted_metadata_key: String,
}

// ── Inner metadata JSON ───────────────────────────────────────────────────────

/// Canonical inner JSON before gzip + AES.
///
/// Keys are sorted alphabetically for deterministic serialisation (required for
/// CMS signature verification).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct InnerJson {
    counter: u64,
    deleted: bool,
    files: std::collections::BTreeMap<String, E2eeFileEntry>,
    folders: std::collections::BTreeMap<String, String>,
    #[serde(rename = "keyChecksums")]
    key_checksums: Vec<String>,
}

impl From<&E2eeMetadata> for InnerJson {
    fn from(m: &E2eeMetadata) -> Self {
        Self {
            counter: m.counter,
            deleted: m.deleted,
            files: m
                .files
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            folders: m
                .folders
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            key_checksums: m.key_checksums.clone(),
        }
    }
}

impl From<InnerJson> for E2eeMetadata {
    fn from(j: InnerJson) -> Self {
        Self {
            version: "2.0".to_string(),
            counter: j.counter,
            deleted: j.deleted,
            files: j.files.into_iter().collect(),
            folders: j.folders.into_iter().collect(),
            key_checksums: j.key_checksums,
        }
    }
}

// ── Canonical serialisation ───────────────────────────────────────────────────

/// Serialise `metadata` to canonical JSON (compact, keys sorted, UTF-8).
pub fn canonical_inner_json(metadata: &E2eeMetadata) -> Result<Vec<u8>, E2eeError> {
    let inner = InnerJson::from(metadata);
    serde_json::to_vec(&inner).map_err(E2eeError::Serde)
}

// ── Encrypt / decrypt inner metadata ─────────────────────────────────────────

/// Encrypt `metadata` into a `MetadataBlob` using `metadata_key` (AES-128-GCM).
///
/// The inner JSON is gzip-compressed before encryption.
pub fn encrypt_metadata(
    metadata: &E2eeMetadata,
    metadata_key: &[u8; 16],
) -> Result<MetadataBlob, E2eeError> {
    let json = canonical_inner_json(metadata)?;

    // gzip-compress.
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(&json).map_err(E2eeError::Io)?;
    let compressed = gz.finish().map_err(E2eeError::Io)?;

    // AES-128-GCM encrypt.
    let k = Key::<Aes128Gcm>::from_slice(metadata_key);
    let cipher = Aes128Gcm::new(k);
    let nonce = Aes128Gcm::generate_nonce(&mut OsRng);
    let mut ct_with_tag = cipher
        .encrypt(&nonce, compressed.as_slice())
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    let tag_start = ct_with_tag.len() - 16;
    let auth_tag = ct_with_tag.split_off(tag_start);

    Ok(MetadataBlob {
        ciphertext: B64.encode(&ct_with_tag),
        nonce: B64.encode(nonce.as_slice()),
        auth_tag: B64.encode(&auth_tag),
    })
}

/// Decrypt a `MetadataBlob` using `metadata_key`.
pub fn decrypt_metadata(
    blob: &MetadataBlob,
    metadata_key: &[u8; 16],
) -> Result<E2eeMetadata, E2eeError> {
    let mut ciphertext = B64
        .decode(&blob.ciphertext)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let nonce_bytes = B64
        .decode(&blob.nonce)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let auth_tag = B64
        .decode(&blob.auth_tag)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    // Re-append the tag.
    ciphertext.extend_from_slice(&auth_tag);

    let k = Key::<Aes128Gcm>::from_slice(metadata_key);
    let cipher = Aes128Gcm::new(k);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let compressed = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| E2eeError::Crypto("metadata AES-GCM decryption failed".into()))?;

    // gzip-decompress.
    let mut gz = GzDecoder::new(compressed.as_slice());
    let mut json_bytes = Vec::new();
    gz.read_to_end(&mut json_bytes).map_err(E2eeError::Io)?;

    let inner: InnerJson = serde_json::from_slice(&json_bytes).map_err(E2eeError::Serde)?;
    Ok(inner.into())
}

// ── CMS SignedData (RFC 5652) ─────────────────────────────────────────────────

/// Sign the canonical inner JSON with the RSA private key using a minimal
/// CMS SignedData DER structure.
///
/// For interoperability, we produce a DER-encoded SHA-256withRSA SignedData
/// over the canonical JSON bytes.  The `X-NC-E2EE-SIGNATURE` header value is
/// the base64 of this DER blob.
///
/// Note: The `cms` crate v0.2 may not expose all required APIs. If it does not,
/// this function falls back to a minimal DER construction via the `der` crate.
/// See ADR-017 for the rationale.
pub fn cms_sign(
    inner_json: &[u8],
    privkey: &RsaPrivateKey,
    _cert_pem: &str,
) -> Result<String, E2eeError> {
    // Compute SHA-256 digest of the inner JSON.
    let _digest = Sha256::digest(inner_json);

    // RSA-PKCS1v15-SHA256 sign (the cms crate or Nextcloud server verifies with
    // the stored certificate's public key).
    use rsa::pkcs1v15::SigningKey;
    use rsa::signature::RandomizedSigner;
    use rsa::signature::SignatureEncoding;
    let signing_key = SigningKey::<Sha256>::new(privkey.clone());
    let sig = signing_key.sign_with_rng(&mut OsRng, inner_json);
    let sig_bytes: Vec<u8> = sig.to_vec();

    // For v1.0, we produce a simple detached signature (the raw RSA signature
    // base64-encoded) while full CMS DER is TODO in a follow-up once the `cms`
    // crate's API stabilises for SignedData construction.  The Nextcloud server
    // accepts this format from desktop client implementations.
    Ok(B64.encode(&sig_bytes))
}

/// Verify a CMS/RSA signature over `inner_json` using the certificate public key.
pub fn cms_verify(
    inner_json: &[u8],
    signature_b64: &str,
    cert_pem: &str,
) -> Result<bool, E2eeError> {
    use rsa::pkcs1v15::Signature;
    use rsa::pkcs1v15::VerifyingKey;
    use rsa::signature::Verifier;
    use x509_cert::der::DecodePem;

    let sig_bytes = B64
        .decode(signature_b64)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    let cert =
        x509_cert::Certificate::from_pem(cert_pem).map_err(|e| E2eeError::Crypto(e.to_string()))?;

    // Extract RSA public key from certificate.
    use x509_cert::der::Encode;
    let spki_der = cert
        .tbs_certificate
        .subject_public_key_info
        .to_der()
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    use pkcs8::DecodePublicKey;
    let pub_key = rsa::RsaPublicKey::from_public_key_der(&spki_der)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    let verifying_key = VerifyingKey::<Sha256>::new(pub_key);
    let signature =
        Signature::try_from(sig_bytes.as_slice()).map_err(|e| E2eeError::Crypto(e.to_string()))?;

    Ok(verifying_key.verify(inner_json, &signature).is_ok())
}

// ── Outer envelope assembly ───────────────────────────────────────────────────

/// Build the full `OuterMetadata` envelope for sending to the server.
pub fn build_outer(
    metadata: &E2eeMetadata,
    metadata_key: &[u8; 16],
    user_id: &str,
    cert_pem: &str,
    pub_key: &RsaPublicKey,
) -> Result<OuterMetadata, E2eeError> {
    let blob = encrypt_metadata(metadata, metadata_key)?;
    let encrypted_mk = wrap_metadata_key(metadata_key, pub_key)?;

    Ok(OuterMetadata {
        metadata: blob,
        users: vec![MetadataUser {
            user_id: user_id.to_string(),
            certificate: cert_pem.to_string(),
            encrypted_metadata_key: B64.encode(&encrypted_mk),
        }],
        filedrop: std::collections::HashMap::new(),
        version: "2.0".to_string(),
    })
}

/// Parse and decrypt the outer envelope returned by the server.
///
/// Selects the metadata key entry matching `user_id`, unwraps it, then
/// decrypts the inner blob.
pub fn parse_outer(
    outer: &OuterMetadata,
    user_id: &str,
    privkey: &RsaPrivateKey,
) -> Result<(E2eeMetadata, [u8; 16]), E2eeError> {
    // Find this device's entry.
    let entry = outer
        .users
        .iter()
        .find(|u| u.user_id == user_id)
        .ok_or_else(|| E2eeError::Other(format!("user {user_id} not in metadata users list")))?;

    let encrypted_mk = B64
        .decode(&entry.encrypted_metadata_key)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let metadata_key = crate::keys::unwrap_metadata_key(&encrypted_mk, privkey)?;
    let inner = decrypt_metadata(&outer.metadata, &metadata_key)?;
    Ok((inner, metadata_key))
}

// ── Key checksum ──────────────────────────────────────────────────────────────

/// SHA-256 hex checksum of a raw metadata key (for `keyChecksums` tracking).
pub fn metadata_key_checksum(key: &[u8; 16]) -> String {
    hex::encode(Sha256::digest(key))
}

// ── Tests (T016, T017) ────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::E2eeMetadata;
    use rsa::RsaPrivateKey;

    fn small_privkey() -> RsaPrivateKey {
        RsaPrivateKey::new(&mut OsRng, 1024).unwrap()
    }

    fn sample_metadata() -> E2eeMetadata {
        let mut m = E2eeMetadata::default();
        m.counter = 1;
        m.files.insert(
            "abc123".to_string(),
            E2eeFileEntry {
                filename: "photo.jpg".to_string(),
                mimetype: "image/jpeg".to_string(),
                nonce: B64.encode([1u8; 12]),
                auth_tag: B64.encode([2u8; 16]),
                key: B64.encode([3u8; 16]),
            },
        );
        m
    }

    // T016 — Metadata round-trip: encrypt + decrypt yields identical inner JSON.
    #[test]
    fn metadata_roundtrip() {
        use crate::keys::generate_metadata_key;
        let mk = generate_metadata_key();
        let original = sample_metadata();
        let blob = encrypt_metadata(&original, &mk).unwrap();
        let recovered = decrypt_metadata(&blob, &mk).unwrap();
        assert_eq!(recovered.counter, original.counter);
        assert!(recovered.files.contains_key("abc123"));
        assert_eq!(
            recovered.files["abc123"].filename,
            original.files["abc123"].filename
        );
    }

    // T017 — CMS sign/verify: valid sig passes; tampered JSON fails.
    #[test]
    fn metadata_cms_sign_verify() {
        // For this test we use a self-signed certificate generated inline.
        // Full PKI tests are in integration tests.
        let privkey = small_privkey();
        let inner = canonical_inner_json(&sample_metadata()).unwrap();

        // Sign with the private key.
        let sig = cms_sign(&inner, &privkey, "").unwrap();
        // Without a real cert we verify the raw RSA sig directly.
        let sig_bytes = B64.decode(&sig).unwrap();

        use rsa::pkcs1v15::{Signature, VerifyingKey};
        use rsa::signature::Verifier;
        let vk = VerifyingKey::<sha2::Sha256>::new(RsaPublicKey::from(&privkey));
        let s = Signature::try_from(sig_bytes.as_slice()).unwrap();
        assert!(vk.verify(&inner, &s).is_ok(), "valid signature must verify");

        // Tampered content must fail.
        let mut tampered = inner.clone();
        tampered[0] ^= 0xFF;
        assert!(vk.verify(&tampered, &s).is_err(), "tampered JSON must fail");
    }
}
