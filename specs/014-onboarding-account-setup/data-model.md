# Data Model: Onboarding — Account Setup Wizard

**Feature**: 014-onboarding-account-setup | **Date**: 2026-06-01

## New Types (Rust / Tauri IPC DTOs)

### ServerProbeDto

Result of probing a candidate Nextcloud server URL.

| Field | Type | Description |
|-------|------|-------------|
| `reachable` | `bool` | Server responded with HTTP 200 |
| `maintenance` | `bool` | Server is in maintenance mode |
| `version` | `String` | Human-readable version string e.g. `"28.0.1"` |
| `version_ok` | `bool` | Major version ≥ 16 (Login Flow v2 required) |
| `e2ee_available` | `bool` | Version ≥ 20 (E2EE stable) |
| `tls_valid` | `bool` | TLS certificate chain trusted |
| `latency_ms` | `u32` | Round-trip time of the status.php probe, ms |
| `error` | `Option<String>` | Human-readable error if `reachable = false` |

---

### AuthFlowInitDto

Returned immediately by `begin_auth_flow`. Contains everything the UI needs to
render step 3 without further polling.

| Field | Type | Description |
|-------|------|-------------|
| `display_code` | `String` | Last 8 chars of the login token, formatted "XXXX · XXXX" |
| `login_url` | `String` | Full URL for the QR code and browser open |
| `qr_svg` | `String` | Inline SVG string of the QR code matrix |
| `expires_at` | `u64` | Unix timestamp (seconds) when the 5-min countdown ends |

---

### AuthFlowState (internal — not exposed over IPC)

Held in `AppState.auth_flow: Mutex<Option<AuthFlowState>>`. Cleared on completion or expiry.

| Field | Type | Description |
|-------|------|-------------|
| `poll_token` | `String` | NC Login Flow v2 poll token |
| `poll_endpoint` | `String` | NC Login Flow v2 poll endpoint URL |
| `server_url` | `String` | Server URL being authenticated against |
| `task_handle` | `JoinHandle<()>` | Background polling task; aborted on cancel |
| `expires_at` | `Instant` | 5-minute absolute deadline |

---

### RemoteStatsDto

Remote account storage quota, returned by `get_account_remote_stats`.

| Field | Type | Description |
|-------|------|-------------|
| `total_bytes` | `u64` | Total quota in bytes (`quota.total` from OCS) |
| `used_bytes` | `u64` | Bytes used (`quota.used` from OCS) |
| `file_count` | `Option<u64>` | Total remote file count; `null` if scan not yet complete |

---

### OnboardingPrefsDto (IPC DTO)

Sent by the frontend in `complete_onboarding`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `local_folder` | `String` | — | Absolute path chosen by user |
| `vfs_enabled` | `bool` | `true` | On-demand files |
| `pin_pinned_folders` | `bool` | `true` | Always keep pinned folders local |
| `smart_bandwidth` | `bool` | `true` | Throttle on battery/metered |
| `watch_external_edits` | `bool` | `false` | Short scan interval |

---

### OnboardingPrefs (persisted — extends SavedConfig)

Stored in `config.json` under key `"onboarding_prefs"`. Serde `default` ensures
backwards compatibility when loading a config written before this feature.

```rust
#[derive(Serialize, Deserialize, Clone, Default)]
pub struct OnboardingPrefs {
    pub vfs_enabled: bool,
    pub pin_pinned_folders: bool,
    pub smart_bandwidth: bool,
    pub watch_external_edits: bool,
}
```

## Modified Types

### SavedConfig

`config.json` gains one optional field:

```rust
pub struct SavedConfig {
    // ... existing fields ...
    #[serde(default)]
    pub onboarding_prefs: OnboardingPrefs,
}
```

Backward compatible: missing field deserialises to `OnboardingPrefs::default()`.

---

## Tauri Event Payloads

### `adagio://auth-flow-complete`

```typescript
{ account: AccountDto }
```

Emitted when the Login Flow v2 poll receives valid credentials. The wizard listens for
this event and auto-advances to step 4.

### `adagio://auth-flow-expired`

```typescript
{}   // empty payload
```

Emitted when the 5-minute countdown elapses without successful auth. The wizard shows
the expiry state and "Try again" button.

---

## Entity Lifecycle

```
OnboardingSession (in-memory, ephemeral)
  │  created: begin_auth_flow called
  │  updated: each successful status.php probe (server_url stored)
  └─ destroyed: auth-flow-complete or auth-flow-expired

ServerProfile (becomes SavedAccount on success)
  │  populated: probe_server returns reachable=true
  └─ promoted: complete_onboarding persists via daemon DaemonRequest::AddAccount

SyncPreferences (OnboardingPrefsDto → OnboardingPrefs)
  │  set: user adjusts toggles in step 4
  └─ persisted: complete_onboarding writes to config.json + applies via daemon

InitialSyncProgress
  │  source: getStatus() IPC (SyncStatusDto)
  └─ displayed: Begin screen, polled every 2 s (reuses existing App.tsx poll)
```
