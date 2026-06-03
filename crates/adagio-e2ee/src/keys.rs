//! E2EE key management: RSA-4096 key generation, X.509 CSR, PBKDF2 mnemonic
//! key wrapping, and RSA/OAEP metadata-key wrapping.
//!
//! # Key hierarchy
//! ```text
//! BIP-39 mnemonic
//!   └─ PBKDF2-HMAC-SHA256 (600 000 itr, 40-byte salt)
//!        └─ 256-bit AES key
//!             └─ AES-256-GCM wraps RSA-4096 private key
//!                  └─ RSA/OAEP wraps 128-bit per-folder metadata key
//!                       └─ AES-128-GCM encrypts inner metadata blob
//!                            └─ AES-128-GCM encrypts file content (per-file key)
//! ```

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes256Gcm, Key, Nonce,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use bip39::Mnemonic;
use hmac::Hmac;
use pbkdf2::pbkdf2;
use pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rand::RngCore;
use rsa::{oaep::Oaep, pkcs8::DecodePrivateKey, RsaPrivateKey, RsaPublicKey};
use sha2::Sha256;
use zeroize::Zeroizing;

use crate::E2eeError;

/// Number of PBKDF2 iterations for mnemonic → AES key derivation.
const PBKDF2_ITERS: u32 = 600_000;
/// Salt length in bytes.
const SALT_LEN: usize = 40;
/// RSA key size in bits.
///
/// The official Nextcloud Desktop, iOS, and Android clients all use RSA-2048.
/// Nextcloud's server-side PHP validation has implicit expectations about the
/// encrypted key size; using RSA-4096 causes POST /meta-data to return 500.
const RSA_BITS: usize = 2048;

// ── Mnemonic generation ───────────────────────────────────────────────────────

/// Generate a fresh 12-word BIP-39 mnemonic.
pub fn generate_mnemonic() -> Result<String, E2eeError> {
    let mnemonic = Mnemonic::generate(12).map_err(|e| E2eeError::InvalidMnemonic(e.to_string()))?;
    Ok(mnemonic.to_string())
}

/// Validate a mnemonic string and return it normalised (lowercase, single spaces).
pub fn validate_mnemonic(mnemonic: &str) -> Result<String, E2eeError> {
    let m = Mnemonic::parse(mnemonic).map_err(|e| E2eeError::InvalidMnemonic(e.to_string()))?;
    Ok(m.to_string())
}

// ── RSA key pair ──────────────────────────────────────────────────────────────

/// Generate an RSA-4096 private key.
///
/// **Note**: this is slow (~1–2 s) and MUST be called from a `spawn_blocking`
/// context so it does not block the Tokio executor.
pub fn generate_rsa_keypair() -> Result<RsaPrivateKey, E2eeError> {
    let mut rng = OsRng;
    RsaPrivateKey::new(&mut rng, RSA_BITS).map_err(|e| E2eeError::Crypto(e.to_string()))
}

/// Serialise an RSA private key to PKCS#8 PEM.
pub fn private_key_to_pem(key: &RsaPrivateKey) -> Result<Zeroizing<String>, E2eeError> {
    key.to_pkcs8_pem(LineEnding::LF)
        .map_err(|e| E2eeError::Crypto(e.to_string()))
}

/// Deserialise an RSA private key from PKCS#8 PEM.
pub fn private_key_from_pem(pem: &str) -> Result<RsaPrivateKey, E2eeError> {
    RsaPrivateKey::from_pkcs8_pem(pem).map_err(|e| E2eeError::Crypto(e.to_string()))
}

/// Extract the public key from a private key and encode as DER bytes.
pub fn public_key_der(privkey: &RsaPrivateKey) -> Vec<u8> {
    let pub_key = RsaPublicKey::from(privkey);
    pub_key
        .to_public_key_der()
        .expect("public key DER encoding failed")
        .to_vec()
}

// ── Private key wrapping (mnemonic → PBKDF2 → AES-256-GCM) ───────────────────

/// Derive a 256-bit AES key from the mnemonic via PBKDF2-HMAC-SHA256.
fn derive_aes_key(mnemonic: &str, salt: &[u8]) -> Zeroizing<[u8; 32]> {
    let mut key = Zeroizing::new([0u8; 32]);
    // Normalise: lowercase, collapse whitespace
    let normalised = mnemonic.trim().to_lowercase();
    pbkdf2::<Hmac<Sha256>>(
        normalised.as_bytes(),
        salt,
        PBKDF2_ITERS,
        key.as_mut_slice(),
    )
    .expect("PBKDF2 infallible");
    key
}

/// Encrypt the RSA private key PEM with AES-256-GCM using a mnemonic-derived key.
///
/// Returns `"{base64(ciphertext)}|{base64(nonce)}|{base64(salt)}"` — the format
/// stored on the Nextcloud server.
pub fn wrap_private_key(privkey: &RsaPrivateKey, mnemonic: &str) -> Result<String, E2eeError> {
    let pem = private_key_to_pem(privkey)?;
    let mut salt = [0u8; SALT_LEN];
    OsRng.fill_bytes(&mut salt);
    let aes_key = derive_aes_key(mnemonic, &salt);
    let k = Key::<Aes256Gcm>::from_slice(aes_key.as_slice());
    let cipher = Aes256Gcm::new(k);
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
    let ciphertext = cipher
        .encrypt(&nonce, pem.as_bytes())
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    Ok(format!(
        "{}|{}|{}",
        B64.encode(&ciphertext),
        B64.encode(nonce.as_slice()),
        B64.encode(salt),
    ))
}

/// Decrypt the server-stored encrypted private key blob using the mnemonic.
///
/// Returns `E2eeError::MnemonicMismatch` if decryption fails (wrong mnemonic).
pub fn unwrap_private_key(blob: &str, mnemonic: &str) -> Result<RsaPrivateKey, E2eeError> {
    let parts: Vec<&str> = blob.splitn(3, '|').collect();
    if parts.len() != 3 {
        return Err(E2eeError::Crypto("malformed private-key blob".into()));
    }
    let ciphertext = B64
        .decode(parts[0])
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let nonce_bytes = B64
        .decode(parts[1])
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let salt = B64
        .decode(parts[2])
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    let aes_key = derive_aes_key(mnemonic, &salt);
    let k = Key::<Aes256Gcm>::from_slice(aes_key.as_slice());
    let cipher = Aes256Gcm::new(k);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext.as_slice())
        .map_err(|_| E2eeError::MnemonicMismatch)?;

    let pem = std::str::from_utf8(&plaintext)
        .map_err(|_| E2eeError::Crypto("decrypted private key is not valid UTF-8".into()))?;
    private_key_from_pem(pem)
}

// ── Metadata key wrapping (RSA/OAEP) ─────────────────────────────────────────

/// Wrap a 128-bit metadata key with RSA/OAEP-SHA-256 using the user's certificate public key.
pub fn wrap_metadata_key(
    metadata_key: &[u8; 16],
    pub_key: &RsaPublicKey,
) -> Result<Vec<u8>, E2eeError> {
    let mut rng = OsRng;
    let padding = Oaep::new::<Sha256>();
    pub_key
        .encrypt(&mut rng, padding, metadata_key)
        .map_err(|e| E2eeError::Crypto(e.to_string()))
}

/// Unwrap a metadata key with the RSA private key.
pub fn unwrap_metadata_key(
    encrypted: &[u8],
    privkey: &RsaPrivateKey,
) -> Result<[u8; 16], E2eeError> {
    let padding = Oaep::new::<Sha256>();
    let decrypted = privkey
        .decrypt(padding, encrypted)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    if decrypted.len() != 16 {
        return Err(E2eeError::Crypto(format!(
            "expected 16-byte metadata key, got {} bytes",
            decrypted.len()
        )));
    }
    let mut key = [0u8; 16];
    key.copy_from_slice(&decrypted);
    Ok(key)
}

/// Generate a fresh random 128-bit metadata key.
pub fn generate_metadata_key() -> [u8; 16] {
    let mut key = [0u8; 16];
    OsRng.fill_bytes(&mut key);
    key
}

// ── PKCS#10 CSR ───────────────────────────────────────────────────────────────

/// Build a PKCS#10 Certificate Signing Request (CSR) in PEM format.
///
/// The CSR has `CN={username}` as the subject and is signed with SHA-256 using
/// the supplied RSA private key.  Nextcloud's E2EE OCS API (`POST …/public-key`)
/// expects a valid PEM-encoded CSR in the `csr` form field.
pub fn build_csr(privkey: &RsaPrivateKey, username: &str) -> Result<String, E2eeError> {
    use rsa::pkcs1v15::SigningKey;
    use std::str::FromStr;
    use x509_cert::{
        builder::{Builder, RequestBuilder},
        der::EncodePem,
        name::Name,
    };

    let subject = Name::from_str(&format!("CN={username}"))
        .map_err(|e| E2eeError::Crypto(format!("CSR subject: {e}")))?;

    let signer = SigningKey::<Sha256>::new(privkey.clone());

    let builder = RequestBuilder::new(subject, &signer)
        .map_err(|e| E2eeError::Crypto(format!("CSR builder: {e}")))?;

    let csr = builder
        .build::<rsa::pkcs1v15::Signature>()
        .map_err(|e| E2eeError::Crypto(format!("CSR sign: {e}")))?;

    csr.to_pem(LineEnding::LF)
        .map_err(|e| E2eeError::Crypto(format!("CSR PEM encode: {e}")))
}

// ── Fingerprint ───────────────────────────────────────────────────────────────

/// Compute a `"sha256:{hex}"` fingerprint of DER-encoded public key bytes.
pub fn key_fingerprint(der: &[u8]) -> String {
    use sha2::Digest;
    let hash = sha2::Sha256::digest(der);
    format!("sha256:{}", hex::encode(hash))
}

// ── Tests (T015) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn test_mnemonic() -> &'static str {
        "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
    }

    // T015 — Generate RSA key, wrap with mnemonic, unwrap, assert round-trip.
    #[test]
    fn key_gen_and_wrap() {
        // Use a smaller key for test speed (still proves the API).
        let privkey = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
        let blob = wrap_private_key(&privkey, test_mnemonic()).unwrap();
        let recovered = unwrap_private_key(&blob, test_mnemonic()).unwrap();
        // Compare moduli as a proxy for key equality.
        use rsa::traits::PublicKeyParts;
        assert_eq!(privkey.n(), recovered.n());
    }

    // T037/T038 — Correct mnemonic succeeds; wrong mnemonic returns MnemonicMismatch.
    #[test]
    fn pair_correct_mnemonic() {
        let privkey = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
        let blob = wrap_private_key(&privkey, test_mnemonic()).unwrap();
        assert!(unwrap_private_key(&blob, test_mnemonic()).is_ok());
    }

    #[test]
    fn pair_wrong_mnemonic_fails() {
        let privkey = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
        let blob = wrap_private_key(&privkey, test_mnemonic()).unwrap();
        let wrong = "zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo zoo wrong";
        let result = unwrap_private_key(&blob, wrong);
        assert!(matches!(result, Err(E2eeError::MnemonicMismatch)));
    }

    #[test]
    fn metadata_key_wrap_roundtrip() {
        let privkey = RsaPrivateKey::new(&mut OsRng, 1024).unwrap();
        let pub_key = RsaPublicKey::from(&privkey);
        let mk = generate_metadata_key();
        let wrapped = wrap_metadata_key(&mk, &pub_key).unwrap();
        let recovered = unwrap_metadata_key(&wrapped, &privkey).unwrap();
        assert_eq!(mk, recovered);
    }

    #[test]
    fn mnemonic_generation() {
        let m = generate_mnemonic().unwrap();
        let words: Vec<&str> = m.split_whitespace().collect();
        assert_eq!(words.len(), 12);
    }
}
