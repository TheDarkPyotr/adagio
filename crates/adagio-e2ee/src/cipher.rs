//! AES-128-GCM file content encryption / decryption.
//!
//! Each file gets a fresh 128-bit key and 96-bit nonce.
//! The AES-GCM authentication tag is stored alongside the ciphertext.

use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Aes128Gcm, Key, Nonce,
};

use crate::E2eeError;

/// Return type for `encrypt_file`: (key, nonce, auth_tag, ciphertext).
pub type EncryptResult = ([u8; 16], [u8; 12], [u8; 16], Vec<u8>);

/// Encrypt `plaintext` with AES-128-GCM.
///
/// Returns `(key_bytes, nonce_bytes, auth_tag_bytes, ciphertext)`.
/// The key, nonce and tag are stored in the metadata; the ciphertext goes to
/// the server as an opaque blob.
pub fn encrypt_file(plaintext: &[u8]) -> Result<EncryptResult, E2eeError> {
    let key = Aes128Gcm::generate_key(&mut OsRng);
    let nonce = Aes128Gcm::generate_nonce(&mut OsRng);
    let cipher = Aes128Gcm::new(&key);

    // AES-GCM appends the 16-byte tag to the ciphertext.
    let mut ct_with_tag = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;

    // Split off the last 16 bytes as the auth tag.
    let tag_start = ct_with_tag.len() - 16;
    let mut auth_tag = [0u8; 16];
    auth_tag.copy_from_slice(&ct_with_tag[tag_start..]);
    ct_with_tag.truncate(tag_start);

    let mut key_arr = [0u8; 16];
    key_arr.copy_from_slice(key.as_slice());
    let mut nonce_arr = [0u8; 12];
    nonce_arr.copy_from_slice(nonce.as_slice());

    Ok((key_arr, nonce_arr, auth_tag, ct_with_tag))
}

/// Encrypt `plaintext` with a supplied key and nonce (for re-encryption).
pub fn encrypt_with(
    key: &[u8; 16],
    nonce: &[u8; 12],
    plaintext: &[u8],
) -> Result<([u8; 16], Vec<u8>), E2eeError> {
    let k = Key::<Aes128Gcm>::from_slice(key);
    let cipher = Aes128Gcm::new(k);
    let nonce_arr = Nonce::from_slice(nonce);
    let mut ct_with_tag = cipher
        .encrypt(nonce_arr, plaintext)
        .map_err(|e| E2eeError::Crypto(e.to_string()))?;
    let tag_start = ct_with_tag.len() - 16;
    let mut auth_tag = [0u8; 16];
    auth_tag.copy_from_slice(&ct_with_tag[tag_start..]);
    ct_with_tag.truncate(tag_start);
    Ok((auth_tag, ct_with_tag))
}

/// Decrypt `ciphertext` with AES-128-GCM.
///
/// `key`, `nonce` and `auth_tag` come from the file's `E2eeFileEntry`.
pub fn decrypt_file(
    key: &[u8; 16],
    nonce: &[u8; 12],
    auth_tag: &[u8; 16],
    ciphertext: &[u8],
) -> Result<Vec<u8>, E2eeError> {
    let k = Key::<Aes128Gcm>::from_slice(key);
    let cipher = Aes128Gcm::new(k);
    let nonce_arr = Nonce::from_slice(nonce);

    // Re-attach the tag so AES-GCM can verify it.
    let mut ct_with_tag = ciphertext.to_vec();
    ct_with_tag.extend_from_slice(auth_tag);

    cipher
        .decrypt(nonce_arr, ct_with_tag.as_slice())
        .map_err(|_| {
            E2eeError::Crypto("AES-GCM decryption failed — bad key or corrupted ciphertext".into())
        })
}

// ── Tests (T014) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T014 — AES-128-GCM round-trip: encrypt then decrypt yields original plaintext.
    #[test]
    fn cipher_roundtrip() {
        let plaintext = b"Hello, Adagio E2EE!";
        let (key, nonce, tag, ct) = encrypt_file(plaintext).unwrap();
        let recovered = decrypt_file(&key, &nonce, &tag, &ct).unwrap();
        assert_eq!(recovered, plaintext);
    }

    #[test]
    fn wrong_key_fails() {
        let plaintext = b"secret";
        let (_, nonce, tag, ct) = encrypt_file(plaintext).unwrap();
        let bad_key = [0u8; 16];
        assert!(decrypt_file(&bad_key, &nonce, &tag, &ct).is_err());
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let (key, nonce, tag, mut ct) = encrypt_file(b"data").unwrap();
        if !ct.is_empty() {
            ct[0] ^= 0xFF;
        }
        assert!(decrypt_file(&key, &nonce, &tag, &ct).is_err());
    }
}
