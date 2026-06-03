# Data Model: End-to-End Encryption (E2EE)

**Branch**: `013-e2ee-encryption` | **Date**: 2026-05-31

---

## New SQLite migrations

### Migration 004 — `e2ee_account_keys`

Stores the per-account E2EE state. One row per Nextcloud account.

```sql
CREATE TABLE IF NOT EXISTS e2ee_account_keys (
    account_id     TEXT NOT NULL PRIMARY KEY,
    -- PEM certificate returned by server CA after CSR submission.
    certificate    TEXT NOT NULL,
    -- Hex-encoded SHA-256 fingerprint of the RSA public key.
    key_fingerprint TEXT NOT NULL,
    -- Monotonically increasing device-side pairing counter (used for diagnostics).
    paired_at      TEXT NOT NULL,   -- ISO-8601 UTC
    created_at     TEXT NOT NULL    -- ISO-8601 UTC
);
```

*The RSA private key is stored in the OS keychain under the key
`"adagio-e2ee/{account_id}"`, never in SQLite.*

---

### Migration 004 — `e2ee_folder_state`

One row per E2EE-enabled sync pair, tracking the metadata sync state.

```sql
CREATE TABLE IF NOT EXISTS e2ee_folder_state (
    pair_id          TEXT NOT NULL PRIMARY KEY,
    -- Nextcloud WebDAV file-ID of the root folder (numeric string).
    folder_id        TEXT NOT NULL,
    -- Metadata format version string as reported by server ("2.0", "1.0", …).
    metadata_version TEXT NOT NULL DEFAULT '2.0',
    -- Last-known server counter value; used to compute X-NC-E2EE-COUNTER.
    counter          INTEGER NOT NULL DEFAULT 0,
    -- Hex-encoded SHA-256 checksums of all metadata keys ever used (space-separated).
    key_checksums    TEXT NOT NULL DEFAULT '',
    updated_at       TEXT NOT NULL   -- ISO-8601 UTC
);
```

---

### Modified table — `sync_pairs` / `SavedPair`

Add two columns to the existing pair persistence (via config.json field addition and a
SQL migration for any SQLite-stored pair state):

| New field | Type | Default | Description |
|-----------|------|---------|-------------|
| `e2ee_enabled` | `bool` | `false` | Whether this pair uses E2EE |
| `e2ee_account_id` | `TEXT` \| `null` | `null` | FK → `e2ee_account_keys.account_id` |

*`e2ee_account_id` is always the same as `pair.account_id`; stored redundantly for
query convenience.*

---

## In-memory types (Rust)

### `E2eeKeySet`

Held in memory by the daemon for the lifetime of an E2EE sync session. Never persisted
to disk.

```rust
pub struct E2eeKeySet {
    pub account_id: AccountId,
    /// RSA-4096 private key (loaded from keychain).
    pub private_key: rsa::RsaPrivateKey,
    /// Signed X.509 certificate (public key + server signature).
    pub certificate: x509_cert::Certificate,
    /// 128-bit AES metadata key for a specific folder.
    pub metadata_key: [u8; 16],
}
```

### `E2eeMetadata`

The decrypted inner metadata blob for a single E2EE folder.

```rust
pub struct E2eeMetadata {
    pub version: String,           // "2.0"
    pub counter: u64,
    pub key_checksums: Vec<String>,
    pub deleted: bool,
    pub files: HashMap<String, E2eeFileEntry>,   // uuid → entry
    pub folders: HashMap<String, String>,         // uuid → cleartext name
}

pub struct E2eeFileEntry {
    pub filename: String,       // original plaintext filename
    pub mimetype: String,
    pub nonce: [u8; 12],        // 96-bit IV
    pub auth_tag: [u8; 16],     // GCM auth tag
    pub key: [u8; 16],          // 128-bit per-file AES key
}
```

### `E2eeProviderTrait`

The trait boundary between `adagio-core` and `adagio-e2ee`.

```rust
#[async_trait]
pub trait E2eeProvider: Send + Sync + 'static {
    /// True when the account has an initialised E2EE key pair on the server.
    async fn is_initialised(&self, account_id: &AccountId) -> Result<bool, E2eeError>;

    /// Generate RSA key pair, submit CSR, upload encrypted private key.
    /// Returns the 12-word BIP-39 mnemonic (shown once; caller is responsible
    /// for presenting to the user and never logging it).
    async fn init(&self, account_id: &AccountId) -> Result<String, E2eeError>;

    /// Download and decrypt the RSA private key using the supplied mnemonic.
    async fn pair(&self, account_id: &AccountId, mnemonic: &str) -> Result<(), E2eeError>;

    /// Encrypt `plaintext` for `pair_id`. Returns (ciphertext, uuid, entry).
    async fn encrypt_file(
        &self,
        pair_id: &PairId,
        filename: &str,
        mimetype: &str,
        plaintext: &[u8],
    ) -> Result<(Vec<u8>, String, E2eeFileEntry), E2eeError>;

    /// Decrypt `ciphertext` for `pair_id` using the entry from metadata.
    async fn decrypt_file(
        &self,
        pair_id: &PairId,
        uuid: &str,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, E2eeError>;

    /// Fetch + decrypt folder metadata from the server; update local counter.
    async fn sync_metadata(&self, pair_id: &PairId) -> Result<E2eeMetadata, E2eeError>;

    /// Lock folder, update metadata with new entry/deletion, unlock folder.
    async fn commit_metadata(
        &self,
        pair_id: &PairId,
        metadata: &E2eeMetadata,
    ) -> Result<(), E2eeError>;

    /// Diagnostic summary for `adagio e2ee status`.
    async fn status(&self, pair_id: &PairId) -> Result<E2eeStatus, E2eeError>;
}
```

---

## IPC extensions

Three new `DaemonRequest` variants (added to `adagio-ipc`):

| Variant | Payload | Response |
|---------|---------|----------|
| `E2eeInit { pair_id }` | — | `{ mnemonic: String }` |
| `E2eePair { pair_id, mnemonic: String }` | — | `Unit` |
| `E2eeStatus { pair_id }` | — | `E2eeStatusDto` |

```rust
// Serialised as JSON; returned by E2eeStatus
pub struct E2eeStatusDto {
    pub pair_id: String,
    pub enabled: bool,
    pub metadata_version: Option<String>,
    pub counter: u64,
    pub key_fingerprint: Option<String>,
    pub encrypted_file_count: u64,
}
```

---

## Key storage layout (OS keychain)

| Keychain key | Contents |
|-------------|----------|
| `adagio-e2ee/{account_id}` | PKCS#8 PEM RSA-4096 private key (plaintext; secured by OS keychain) |
| `adagio/{account_id}` | Nextcloud credentials (existing; unchanged) |

---

## State transitions for a sync pair

```
        ┌─────────────────────────────────────────────────────┐
        │                     SyncPair                         │
        │                                                      │
        │  e2ee_enabled = false                               │
        │        │                                             │
        │  adagio e2ee init / UI toggle                       │
        │        ▼                                             │
        │  e2ee_enabled = true                                │
        │  e2ee_folder_state row created                      │
        │        │                                             │
        │  sync cycle                                         │
        │        ▼                                             │
        │  LOCKED → encrypt files → update metadata → UNLOCK  │
        │        │                                             │
        │  disable E2EE (UI)                                  │
        │        ▼                                             │
        │  Re-upload all files plaintext → delete metadata    │
        │  e2ee_enabled = false                               │
        └─────────────────────────────────────────────────────┘
```
