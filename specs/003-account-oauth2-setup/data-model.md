# Data Model: Account Setup UI — Nextcloud OAuth2 Login Flow

**Feature**: 003-account-oauth2-setup
**Date**: 2026-05-25

---

## Entities

### Account *(persisted in `config.json`)*

Already defined in `crates/adagio-core/src/types.rs`. No structural changes required.

| Field | Type | Notes |
|-------|------|-------|
| `id` | `AccountId` (UUID string) | Primary key |
| `display_name` | `String` | Fetched from `/ocs/v2.php/cloud/user` after auth |
| `server_url` | `String` | Normalised (trailing slash stripped) |
| `username` | `String` | `data.id` from OCS user endpoint |
| `keychain_service_key` | `String` | `account_id` — keychain lookup key; **never** the token |
| `created_at` | `DateTime<Utc>` | |

**Validation rules**:
- `server_url` must parse as an `https://` or `http://` URL (allow http for local dev).
- `server_url` + `username` combination must be unique across all saved accounts.
- `keychain_service_key` equals `account_id.0` (UUID); it is set by the system, not the user.

**State transitions**: Account has no lifecycle state of its own. Removal cascades to sync pairs (enforced by `remove_account` command).

---

### Credential *(stored in OS keychain only)*

Not a Rust struct — stored exclusively via `keyring::Entry::new("adagio", account_id)`.

| Keychain field | Value |
|----------------|-------|
| Service | `"adagio"` |
| Account / username | `account_id` (UUID string) |
| Password / secret | JSON-encoded `TokenPair` |

**Never appears in**: `config.json`, log output, `Account` struct, IPC return values, or any file on disk.

---

### TokenPair *(in-memory + keychain blob)*

Defined in `crates/adagio-nextcloud/src/auth.rs`.

| Field | Type | Notes |
|-------|------|-------|
| `access_token` | `String` | Bearer token for Nextcloud API calls |
| `refresh_token` | `String` | Used to obtain a new access token on 401 |

Serialised as `{"access_token":"...","refresh_token":"..."}` for keychain storage.

---

### AuthSession *(transient, in-memory only)*

Represents one in-progress OAuth2 authorisation attempt. Discarded on success or timeout.

| Field | Type | Notes |
|-------|------|-------|
| `server_url` | `String` | Server being connected |
| `client_id` | `String` | App OAuth2 client ID |
| `redirect_uri` | `String` | `http://127.0.0.1:{port}/callback` |
| `code_verifier` | `String` | PKCE verifier (43–128 chars, base64url) |
| `state` | `String` | CSRF nonce (32 random bytes, base64url) |
| `listener_port` | `u16` | Ephemeral TCP port for the callback |
| `started_at` | `Instant` | For 5-minute timeout enforcement |

`AuthSession` is never serialised. It lives only in the async task executing the flow.

---

## Relationships

```
Account 1 ──── N  SyncPair        (existing)
Account 1 ──── 1  Credential      (keychain, keyed by account_id)
Account 1 ──── 0..1 AuthSession   (transient; exists only during auth)
```

---

## SavedAccount *(config.json shadow)*

Defined in `crates/adagio-desktop/src/config/mod.rs`. Mirrors `Account` fields without
any credential data.

| Field | Type |
|-------|------|
| `id` | `String` |
| `display_name` | `String` |
| `server_url` | `String` |
| `username` | `String` |
| `keychain_service_key` | `String` |

No changes required to this struct for this feature.
