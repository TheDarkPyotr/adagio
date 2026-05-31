# ADR-017: E2EE Protocol — Nextcloud v2.x, RSA-4096 + AES-128-GCM + CMS

**Status**: Accepted  
**Date**: 2026-05-31  
**Branch**: `013-e2ee-encryption`

---

## Context

Adagio adds end-to-end encryption for Nextcloud sync pairs. The implementation must
interoperate with the official Nextcloud Desktop, Android, and iOS clients. The
Nextcloud E2EE RFC (current master) defines the only published protocol.

---

## Decision

### Cryptographic primitives

| Purpose | Algorithm | Rationale |
|---------|-----------|-----------|
| Device key pair | RSA-4096 | Mandated by Nextcloud OCS API (CSR + signed cert) |
| Metadata key wrapping | RSA/OAEP-SHA-256 | Mandated by RFC |
| File content encryption | AES-128-GCM (128-bit key, 96-bit nonce) | RFC mandates 128-bit; interop requirement |
| Private key encryption at rest | AES-256-GCM (256-bit key) | RFC mandates 256-bit for PBKDF2 output |
| Key derivation (mnemonic → AES) | PBKDF2-HMAC-SHA256, 600 000 iterations, 40-byte salt | RFC mandates this exact KDF |
| Mnemonic | BIP-39, 12 words (English) | RFC mandates BIP-39 |
| Metadata integrity | CMS SignedData (RFC 5652, detached) over canonical inner JSON | Mandatory since v2.0 |
| Inner metadata compression | gzip before AES encryption | Specified in RFC |

### Correction from feature proposal

The feature proposal (`spec.md`) stated "AES-256-GCM for content encryption" and
"ed25519-dalek for metadata signatures". Both are incorrect against the published RFC:

- File content keys are **128-bit**, not 256-bit. Using 256-bit would be incompatible
  with all other Nextcloud clients.
- Metadata signing uses **CMS/RSA-4096**, not Ed25519. `ed25519-dalek` is not needed.

### Rust crates chosen

| Crate | Version | Purpose |
|-------|---------|---------|
| `rsa` | 0.9 | RSA-4096 key generation, OAEP-SHA-256 wrap/unwrap |
| `x509-cert` | 0.2 | X.509 CSR construction + certificate parsing |
| `pkcs8` | 0.10 | PKCS#8 private key serialisation |
| `aes-gcm` | 0.10 | AES-128-GCM (files) + AES-256-GCM (private key) |
| `bip39` | 2 | Mnemonic generation + validation |
| `pbkdf2` + `hmac` + `sha2` | latest | KDF for mnemonic → AES key |
| `cms` | 0.2 | CMS SignedData for metadata signature |
| `flate2` | 1 | gzip compression of inner metadata |
| `zeroize` | 1 | Zeroise key material after use |

### Architecture

A new `adagio-e2ee` workspace crate exposes the `E2eeProvider` trait. The core sync
engine calls this trait; non-E2EE sync paths are unchanged. The Nextcloud OCS API calls
are implemented in `adagio-nextcloud::e2ee_ocs`.

---

## Consequences

- **Positive**: Full interoperability with official Nextcloud clients; security properties
  match the audited (post-CVE GHSA-8875-wxww-3rr8) v2.x specification.
- **Negative**: RSA-4096 key generation is slow (~1–2 s on a typical machine); this is
  acceptable as it only happens once per device.
- **Neutral**: The `cms` crate is relatively young; if it lacks required features, DER
  will be constructed manually via the `der` crate as a fallback.
