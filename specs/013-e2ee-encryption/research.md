# Research: End-to-End Encryption (E2EE)

**Branch**: `013-e2ee-encryption` | **Date**: 2026-05-31

---

## Decision 1: Nextcloud E2EE Protocol Version

**Decision**: Implement Nextcloud E2EE **v2.x** (metadata `"version": "2.0"`) as the write
target; support v1.x as read-only.

**Rationale**: v2 is the current secure version. v1 had a missing metadata-key authenticity
check (CVE GHSA-8875-wxww-3rr8) patched by the introduction of CMS SignedData in v2.
All official Nextcloud clients (Desktop 3.8.0+, Android/iOS 4.8.0+) moved to v2.

**Alternatives considered**: v1-only (rejected — insecure); v3 (does not exist yet).

---

## Decision 2: RSA-4096 key pair per user (not ECDH)

**Decision**: Each device generates an **RSA-4096** key pair. The public key is submitted
as an X.509 CSR; the Nextcloud server acts as a CA and signs it. The signed certificate
is stored on the server and retrieved by other devices for key distribution.

**Rationale**: This is what the Nextcloud OCS API requires. The server
`POST /ocs/v2.php/apps/end_to_end_encryption/api/v2/public-key` accepts a PKCS#10 CSR
and returns a signed PEM certificate. `RSA/ECB/OAEPWithSHA-256AndMGF1Padding` is used
for metadata-key wrapping.

**Alternatives considered**: ECDH (rejected — server API does not support it); pre-shared
key (rejected — breaks multi-device).

**Rust crates**: `rsa` (RSA-4096, OAEP-SHA256), `x509-cert` (CSR + certificate),
`pkcs8` (PKCS#8 private key serialisation).

---

## Decision 3: File content encryption — AES-128-GCM with per-file key

**Decision**: Each file is encrypted with **AES-128-GCM** using a fresh random 128-bit key
and 96-bit nonce. The per-file key and nonce are stored inside the encrypted metadata blob.
The server WebDAV filename is a UUID (32 hex chars, no dashes, no extension) — the real
filename lives only inside the encrypted metadata.

**Rationale**: The Nextcloud E2EE RFC mandates 128-bit AES-GCM for file content keys (not
256-bit). Both metadata key and per-file key are 128-bit. The
`aes-gcm` RustCrypto crate covers both.

**Note**: The original feature description says "AES-256-GCM for content encryption".
This is **incorrect** against the published RFC — the protocol uses 128-bit keys for file
content. The 256-bit AES-GCM is used only for encrypting the RSA private key at rest
(see Decision 4). The implementation MUST follow the RFC to interoperate with other
Nextcloud clients.

**Alternatives considered**: AES-256-GCM for files (rejected — not what the RFC specifies;
would be incompatible with server and other clients); ChaCha20-Poly1305 (rejected — not
in the RFC).

**Rust crate**: `aes-gcm` (`Aes128Gcm` for files/metadata, `Aes256Gcm` for private-key
encryption).

---

## Decision 4: BIP-39 mnemonic → PBKDF2 → AES-256-GCM wraps RSA private key

**Decision**: The 12-word BIP-39 mnemonic is the PBKDF2-HMAC-SHA256 passphrase (600,000
iterations, 40-byte random salt) used to derive a 256-bit AES key. That AES key protects
the RSA private key via AES-256-GCM. The encrypted private key blob (`{ciphertext}|{nonce}|{salt}`, all base64) is stored on the Nextcloud server at
`POST /ocs/v2.php/apps/end_to_end_encryption/api/v2/private-key`.

**Rationale**: This is the protocol specification. The mnemonic is **not** a backup of the
private key — it is the passphrase that encrypts it. The same private key is shared across
all user devices via the server store; the mnemonic unlocks it on each device.

**Device pairing flow** (revised from spec):
1. Device B `GET .../private-key` → downloads AES-GCM-encrypted RSA private key blob.
2. User enters 12-word mnemonic (or scans QR from device A).
3. PBKDF2 derives AES-256 key from mnemonic.
4. AES-256-GCM decrypts the RSA private key.
5. Device B verifies private key matches the stored public certificate.
6. Device B stores decrypted private key in OS keychain.

No server-mediated key exchange is required — the mnemonic is sufficient.

**Rust crates**: `bip39` (mnemonic), `pbkdf2` + `hmac` + `sha2` (KDF), `aes-gcm` (key
wrapping).

---

## Decision 5: CMS SignedData (RFC 5652) for metadata signing

**Decision**: Every metadata write (`PUT .../meta-data/{fileId}`) MUST include a
`X-NC-E2EE-SIGNATURE` header containing a base64-encoded CMS SignedData (RFC 5652,
detached, over the canonical inner-metadata JSON). The signature uses the device's RSA
private key; the server verifies against the user's certificate.

**Rationale**: This is mandatory in v2. It replaced the absent integrity check of v1 that
allowed an untrusted server to swap metadata keys (CVE GHSA-8875-wxww-3rr8).

**Note**: The original feature description says "ed25519-dalek for metadata signatures".
This is **incorrect** — the protocol requires CMS over RSA-4096, not Ed25519.
`ed25519-dalek` is NOT needed.

**Rust crate**: `cms` crate (RFC 5652 CMS SignedData); falls back to constructing the
DER structure manually via `der` + `rsa` if the `cms` crate lacks needed functionality.

---

## Decision 6: Metadata v2.x structure

The metadata stored on the server has an **outer envelope** and an **inner encrypted blob**:

```
Outer (JSON sent to/from server):
{
  "metadata": {
    "ciphertext": "<base64>",   // inner JSON, gzipped then AES-128-GCM encrypted
    "nonce":       "<base64>",  // 96-bit IV
    "authenticationTag": "<base64>"
  },
  "users": [
    {
      "userId": "alice",
      "certificate": "<PEM>",
      "encryptedMetadataKey": "<base64>"  // RSA/OAEP-wrapped 128-bit metadata key
    }
  ],
  "filedrop": { ... },   // out of scope for v1.0
  "version": "2.0"
}

Inner (decrypted, gzip-decompressed):
{
  "keyChecksums": ["<sha256-hex>", ...],
  "deleted": false,
  "counter": 42,
  "folders": { "<uuid>": "subfolder-name" },
  "files": {
    "<uuid>": {
      "filename": "photo.jpg",
      "mimetype": "image/jpeg",
      "nonce":    "<base64>",    // 96-bit IV
      "authenticationTag": "<base64>",
      "key":      "<base64>"    // 128-bit per-file AES key
    }
  }
}
```

Canonical inner JSON: compact, keys sorted alphabetically, no trailing whitespace,
UTF-8, gzipped before AES encryption.

---

## Decision 7: Folder lock / counter protocol

All metadata writes require:
1. `POST .../lock/{folderId}` with `X-NC-E2EE-COUNTER: {n+1}` → server returns `e2e-token`.
2. All WebDAV uploads within the locked window.
3. `PUT .../meta-data/{folderId}` with `e2e-token` header and updated metadata.
4. `DELETE .../lock/{folderId}` with `e2e-token` header.

The server rejects lock attempts where the counter is not exactly `current + 1`, preventing
replay attacks. The client MUST persist the current counter locally (database) and refresh
it from the server's metadata on each lock acquisition.

---

## Decision 8: New `adagio-e2ee` crate

**Decision**: Implement E2EE in a new `crates/adagio-e2ee` workspace crate. The existing
`adagio-nextcloud` client gains a thin E2EE OCS API layer; `adagio-core` gains a
`e2ee_enabled` flag on `SyncPair` and routes E2EE pairs through the new crate.

**Rationale**: Constitution principle IV (Extensibility) requires independent crates per
integration. E2EE logic must not pollute the core sync engine.

---

## Decision 9: Scope constraints confirmed

The following are **out of scope for v1.0**:
- Key rotation (changing mnemonic without full re-encryption)
- `filedrop` (guest uploads)
- Sharing E2EE folders with other Nextcloud users
- Headless QR code display (CLI uses typed mnemonic only)
- Per-pair separate mnemonic (single mnemonic per account, shared across all E2EE pairs)
- Support for multiple concurrent users on the same folder (single `users` entry)

---

## Open questions (resolved)

| Question | Resolution |
|----------|-----------|
| AES-256 or AES-128 for files? | AES-128-GCM per RFC; AES-256-GCM only for private-key wrapping |
| Ed25519 or RSA for signing? | CMS/RSA-4096 per RFC; ed25519-dalek not needed |
| Separate pairing API? | No; second device downloads encrypted private key and decrypts with mnemonic |
| Per-pair or per-account mnemonic? | Per-account: one RSA key pair per user, shared via server-stored encrypted blob |
| PBKDF2 iterations? | 600,000 (SHA-256); fall back to SHA-1 / 1,000 iterations for v1 key import |
