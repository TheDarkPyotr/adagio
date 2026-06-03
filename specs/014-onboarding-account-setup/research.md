# Research: Onboarding — Account Setup Wizard

**Feature**: 014-onboarding-account-setup | **Date**: 2026-06-01

## 1. Nextcloud Login Flow v2

**Decision**: Use Nextcloud Login Flow v2 for the onboarding wizard auth step.

### Protocol

```
POST /index.php/login/v2
  → { login: "<url-for-browser>", poll: { token: "<token>", endpoint: "<url>" } }

Browser navigates to `login` URL; user authenticates.

POST <endpoint>   (Content-Type: application/x-www-form-urlencoded)
  Body: token=<token>
  → 404 (not yet) | 200 { server, loginName, appPassword }
```

The client polls `endpoint` with `token` until either the server delivers credentials or the
session expires (Nextcloud default: 20 minutes, but we use our own 5-minute UI countdown).
The `login` URL is what the browser opens and what the QR code encodes.

The displayable "code" shown in the UI is the last path segment of the `login` URL
(e.g. `https://cloud.example.com/index.php/login/v2/flow/<code>`).
Nextcloud generates a short alphanumeric token there (~32 chars). We truncate or format
the last 8 characters for display (e.g. "F4PS9TRX" → shown as "F4PS · 9TRX").

**Rationale**: Login Flow v2 is universally available on NC ≥ 16, works without OAuth2 app
configuration, and provides a displayable code natively.

**Reference**: Nextcloud documentation — "Login Flow v2" (login/v2 endpoint).

### Polling strategy

The polling task runs in a `tokio::spawn` background task inside the desktop process.
It polls every 2 seconds. On success it emits `adagio://auth-flow-complete`. On expiry
(either 5-minute UI countdown or HTTP 410 from server) it emits `adagio://auth-flow-expired`.

The poll task handle and token are stored in `AppState.auth_flow: Mutex<Option<AuthFlowState>>`.
`begin_auth_flow` sets this; `auth-flow-complete` / `auth-flow-expired` clears it.

## 2. Server Probe Endpoint

**Decision**: Use `GET /status.php` (unauthenticated) for capabilities; extend with `GET /ocs/v1.php/config` for E2EE flag.

### Protocol

`GET <server>/status.php` → JSON:
```json
{ "installed": true, "maintenance": false, "needsDbUpgrade": false,
  "version": "28.0.1.3", "versionstring": "28.0.1", ... }
```

This is unauthenticated, fast (< 200 ms on LAN), and available since NC 9.
We derive:
- Reachability: HTTP 200 received
- Version: `versionstring` field
- Maintenance mode: `maintenance` field → show warning
- Minimum version check: major ≥ 16 required for Login Flow v2

E2EE availability is inferred from version ≥ 20 (when NC E2EE became stable).

TLS validity: checked by `reqwest` with `rustls-tls` — connection error if certificate invalid.
Latency: measured as round-trip time of the `status.php` call.

**Debounce**: The UI debounces the URL input by 600 ms before calling `probe_server`.

## 3. QR Code Generation

**Decision**: `qrcode` crate → render as SVG string → pass to frontend.

The `qrcode` crate generates a `QrCode` matrix. We render it as a minimal inline SVG:
```svg
<svg xmlns="…" viewBox="0 0 N N">
  <rect fill="white" width="N" height="N"/>
  <!-- one <rect> per dark module -->
</svg>
```

The SVG is returned as a plain `String` from `begin_auth_flow` and rendered via
`<div dangerouslySetInnerHTML={{ __html: qrSvg }}/>` inside a fixed-size container.

**Input to QR**: the full `login` URL from Login Flow v2 response (≤ 200 chars; fits in QR version 10-12, error correction L).

**Security note**: The URL is server-issued and contains only alphanumeric + URL-safe chars;
no sanitisation needed before SVG embedding (the value is in attribute position, not innerHTML text).

## 4. Native Folder Picker

**Decision**: `tauri-plugin-dialog` 2.x, `open()` with `directory: true`.

Add to `Cargo.toml`:
```toml
tauri-plugin-dialog = "2"
```

Register in `lib.rs`:
```rust
.plugin(tauri_plugin_dialog::init())
```

In `commands/onboarding.rs`:
```rust
use tauri_plugin_dialog::DialogExt;

#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.map(|p| p.to_string_lossy().into_owned()))
}
```

**Platform behaviour**:
- Linux: GTK file chooser dialog
- macOS: NSOpenPanel
- Windows: IFileOpenDialog (Vista+)

All three are available without additional OS configuration.

## 5. Sync Preferences Persistence

**Decision**: Extend `SavedConfig` with `onboarding_prefs: OnboardingPrefs` (serde default = safe defaults).

```rust
#[derive(Serialize, Deserialize, Default, Clone)]
pub struct OnboardingPrefs {
    pub vfs_enabled: bool,          // on-demand toggle → pair.vfs_enabled
    pub pin_pinned_folders: bool,   // passed to VFS runner config
    pub smart_bandwidth: bool,      // if true: set on_battery=throttle network policy
    pub watch_external_edits: bool, // if true: scan_interval_secs=30, else 7200
}
```

`complete_onboarding` reads these and:
1. Creates the sync pair via daemon `DaemonRequest::CreatePair` with `vfs_enabled` and derived `scan_interval_secs`.
2. If `smart_bandwidth`: calls `DaemonRequest::SetNetworkPolicy { on_battery: "throttle", throttle_kbps: 500 }`.
3. Saves `onboarding_prefs` to `config.json`.

**Default values** (matching the UI toggle defaults):
- `vfs_enabled: true` (On-demand ON)
- `pin_pinned_folders: true` (Pin pinned ON)
- `smart_bandwidth: true` (Smart bandwidth ON)
- `watch_external_edits: false` (Watch external OFF)

## 6. Remote Stats for Begin Screen

**Decision**: Use existing `list_remote_tree` daemon call + quota OCS endpoint.

`get_account_remote_stats(account_id)`:
1. Calls `GET /ocs/v2.php/cloud/users/{username}` (Bearer token) → parse `data.quota.total` and `data.quota.used`.
2. Returns `{ file_count: null, total_bytes, used_bytes }` immediately.
   File count is populated asynchronously via `list_remote_tree` once the pair scan runs.

The Begin screen shows:
- `total_bytes` formatted (e.g. "86.7 GB total quota")
- Live sync progress from `getStatus()` polled every 2 s (already in App.tsx)

If `file_count` is null when the Begin screen loads, show a spinner; update when available via the 2 s status poll (`active_file_count` from `SyncStatusDto`).

## 7. ADR — Login Flow v2 vs PKCE

**File**: `docs/adr/014-login-flow-v2.md`

Key points to record:
- Existing `connect_account_oauth2` (PKCE loopback) remains for non-wizard paths
- Wizard uses Login Flow v2 exclusively
- Migration: none needed; both flows store credentials identically in keychain
- Implications: wizard no longer requires OAuth2 app to be configured on the server
