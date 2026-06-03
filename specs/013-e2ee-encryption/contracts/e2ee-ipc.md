# IPC Contract: E2EE Operations

**Branch**: `013-e2ee-encryption` | **Date**: 2026-05-31

All messages follow the existing NDJSON IPC transport (ADR-009). New variants are
appended to the `DaemonRequest` / `DaemonResponse` enums in `crates/adagio-ipc/src/types.rs`.

---

## E2eeInit

Initialises E2EE for the specified sync pair. Generates an RSA-4096 key pair, submits a
CSR to the Nextcloud server, uploads the mnemonic-protected private key, and marks the
folder as E2EE-encrypted via the OCS API.

The returned mnemonic MUST be displayed to the user **exactly once** and never logged.

**Request**

```json
{
  "E2eeInit": {
    "pair_id": "b39ef811-d6e7-4097-991a-2b95c32aeb31"
  }
}
```

**Response (success)**

```json
{
  "E2eeInit": {
    "mnemonic": "word1 word2 word3 word4 word5 word6 word7 word8 word9 word10 word11 word12"
  }
}
```

**Error cases**

| Condition | Error message |
|-----------|--------------|
| Pair not found | `"pair not found: {pair_id}"` |
| Account credentials unavailable | `"credentials unavailable for account {account_id}"` |
| Server already has a key for this account | `"E2EE already initialised; use pair to add this device"` |
| Server rejected CSR | `"server rejected public key: {reason}"` |
| Folder already E2EE on server (unknown key) | `"folder is already E2EE-encrypted by another key; pair first"` |
| Metadata version not recognised | `"unsupported metadata_ver {ver}; upgrade adagio"` |

---

## E2eePair

Downloads the encrypted RSA private key from the server and decrypts it using the
supplied mnemonic. After success the device can encrypt/decrypt E2EE files.

**Request**

```json
{
  "E2eePair": {
    "pair_id": "b39ef811-d6e7-4097-991a-2b95c32aeb31",
    "mnemonic": "word1 word2 ... word12"
  }
}
```

**Response (success)**

```json
{ "Unit": {} }
```

**Error cases**

| Condition | Error message |
|-----------|--------------|
| Mnemonic invalid (not 12 BIP-39 words) | `"invalid mnemonic: expected 12 BIP-39 words"` |
| Mnemonic correct words but wrong key | `"mnemonic does not match the stored key; check spelling"` |
| No private key on server | `"no E2EE key found on server; run init first"` |
| Server certificate chain invalid | `"server certificate validation failed"` |

---

## E2eeStatus

Returns the current E2EE state for a pair.

**Request**

```json
{
  "E2eeStatus": {
    "pair_id": "b39ef811-d6e7-4097-991a-2b95c32aeb31"
  }
}
```

**Response**

```json
{
  "E2eeStatus": {
    "pair_id": "b39ef811-d6e7-4097-991a-2b95c32aeb31",
    "enabled": true,
    "metadata_version": "2.0",
    "counter": 42,
    "key_fingerprint": "sha256:abcd1234...",
    "encrypted_file_count": 127
  }
}
```

Field `metadata_version` and `key_fingerprint` are `null` when E2EE is not yet
initialised or this device has not paired.

---

## Modified: GetStatus

The existing `GetStatus` response gains a new optional field `e2ee_pairs` listing which
pair IDs have E2EE enabled:

```json
{
  "status": "idle",
  "active_file_count": 0,
  "total_bytes": 0,
  "transferred_bytes": 0,
  "eta_seconds": null,
  "last_sync_at": null,
  "e2ee_pairs": ["b39ef811-d6e7-4097-991a-2b95c32aeb31"]
}
```

Clients that do not expect this field ignore it (JSON forward-compatibility).

---

## CLI command schema

All three subcommands support `--json` for machine-readable output.

### `adagio e2ee init [--pair <id>]`

```
adagio e2ee init
adagio e2ee init --pair b39ef811-d6e7-4097-991a-2b95c32aeb31
adagio e2ee init --json
```

**stdout (human)**
```
E2EE initialised for pair "My Cloud" (b39ef811…)
Your mnemonic (write it down — shown only once):

  word1 word2 word3 word4 word5 word6
  word7 word8 word9 word10 word11 word12

Keep this mnemonic safe. It is the only way to access your encrypted files on another device.
```

**stdout (--json)**
```json
{"ok": true, "mnemonic": "word1 word2 ... word12", "pair_id": "b39ef811..."}
```

Exit 0 on success; 1 on error (error JSON on stderr with `--json`).

---

### `adagio e2ee pair [--pair <id>] [--qr] [--mnemonic <words>]`

```
adagio e2ee pair
adagio e2ee pair --pair b39ef811… --mnemonic "word1 word2 ... word12"
adagio e2ee pair --json
```

Interactive mode prompts for the mnemonic securely (no echo).
`--qr` is only available in GUI mode (ignored in headless mode with a warning).

**stdout (--json)**
```json
{"ok": true, "pair_id": "b39ef811...", "key_fingerprint": "sha256:abcd1234..."}
```

---

### `adagio e2ee status [--pair <id>]`

```
adagio e2ee status
adagio e2ee status --pair b39ef811…
adagio e2ee status --json
```

**stdout (human)**
```
E2EE Status
  Pair:             My Cloud (b39ef811…)
  Enabled:          yes
  Metadata version: 2.0
  Counter:          42
  Key fingerprint:  sha256:abcd1234…
  Encrypted files:  127
```

**stdout (--json)** — matches `E2eeStatusDto` schema above.
