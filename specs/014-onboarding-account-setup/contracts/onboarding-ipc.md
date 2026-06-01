# IPC Contract: Onboarding — Account Setup Wizard

**Feature**: 014-onboarding-account-setup | **Date**: 2026-06-01

This document defines all new Tauri IPC commands and events introduced by the onboarding feature.
Existing commands (`list_accounts`, `get_status`, `set_network_policy`, etc.) are unchanged.

---

## Commands (UI → Backend)

### `probe_server`

Probe a candidate Nextcloud URL to determine reachability, version, and capabilities.
Called on a 600 ms debounce from the URL input on step 2.

**Request**
```typescript
invoke('probe_server', { serverUrl: string })
```

**Response** — `ServerProbeDto`
```typescript
{
  reachable: boolean;         // false if network error, TLS error, or non-200
  maintenance: boolean;       // true if server reports maintenance mode
  version: string;            // e.g. "28.0.1"
  version_ok: boolean;        // major >= 16
  e2ee_available: boolean;    // version >= 20
  tls_valid: boolean;         // false if certificate untrusted
  latency_ms: number;         // round-trip ms
  error: string | null;       // human-readable if reachable=false
}
```

**Error path**: Returns `Err(String)` only for unexpected panics. All expected failure states
(unreachable, bad TLS, wrong version) are encoded in the DTO with `reachable: false` and a
populated `error` field so the UI can render inline feedback without try/catch.

**Rust signature**
```rust
#[tauri::command]
pub async fn probe_server(server_url: String) -> Result<ServerProbeDto, String>
```

---

### `begin_auth_flow`

Start a Nextcloud Login Flow v2 session. Returns immediately with the displayable code
and QR SVG. A background task begins polling; it emits events on completion or expiry.
Calling this again while a session is active aborts the previous session silently.

**Request**
```typescript
invoke('begin_auth_flow', { serverUrl: string })
```

**Response** — `AuthFlowInitDto`
```typescript
{
  display_code: string;   // formatted "XXXX · XXXX" (last 8 chars of login token)
  login_url: string;      // full URL opened in browser + encoded in QR
  qr_svg: string;         // inline SVG; render with dangerouslySetInnerHTML
  expires_at: number;     // Unix timestamp (seconds); use for countdown
}
```

**Side effects**:
- Opens the user's default browser to `login_url` (via `tauri_plugin_shell`).
- Spawns a background polling task that emits:
  - `adagio://auth-flow-complete` — on success
  - `adagio://auth-flow-expired` — on 5-minute timeout

**Error**: Returns `Err(String)` if the server is unreachable or Login Flow v2 is unsupported.

**Rust signature**
```rust
#[tauri::command]
pub async fn begin_auth_flow(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    server_url: String,
) -> Result<AuthFlowInitDto, String>
```

---

### `pick_folder`

Open the OS-native folder-picker dialog. Returns the chosen absolute path, or `null` if the user
cancels.

**Request**
```typescript
invoke('pick_folder')
```

**Response**
```typescript
string | null    // absolute path or null on cancel
```

**Rust signature**
```rust
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String>
```

---

### `get_account_remote_stats`

Fetch remote quota and (optionally) file count for a connected account.
Called once after auth completes, queued before the Begin screen loads.

**Request**
```typescript
invoke('get_account_remote_stats', { accountId: string })
```

**Response** — `RemoteStatsDto`
```typescript
{
  total_bytes: number;        // quota total in bytes
  used_bytes: number;         // bytes used
  file_count: number | null;  // null until remote scan completes
}
```

**Error**: Returns `Err(String)` if credentials not found or OCS request fails.

**Rust signature**
```rust
#[tauri::command]
pub async fn get_account_remote_stats(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<RemoteStatsDto, String>
```

---

### `complete_onboarding`

Finalise onboarding: create the first sync pair, persist preferences, apply network policy.
Called when the user clicks "Open Adagio" on step 5 (or on step 4 Continue if they skip step 5).

**Request**
```typescript
invoke('complete_onboarding', {
  accountId: string,
  prefs: {
    local_folder: string,
    vfs_enabled: boolean,
    pin_pinned_folders: boolean,
    smart_bandwidth: boolean,
    watch_external_edits: boolean,
  }
})
```

**Response** — `PairDto` (existing type, same as `create_pair` response)

**Side effects**:
- Sends `DaemonRequest::CreatePair` to create the pair with `local_root`, `vfs_enabled`, and
  `scan_interval_secs` derived from `watch_external_edits` (30 s if true, 7200 s if false).
- If `smart_bandwidth = true`: sends `DaemonRequest::SetNetworkPolicy { on_battery: throttle, throttle_kbps: 500 }`.
- Writes `onboarding_prefs` to `config.json`.

**Error**: Returns `Err(String)` if pair creation fails (e.g. local folder not writable).

**Rust signature**
```rust
#[tauri::command]
pub async fn complete_onboarding(
    state: State<'_, AppState>,
    account_id: String,
    prefs: OnboardingPrefsDto,
) -> Result<PairDto, String>
```

---

## Events (Backend → UI)

### `adagio://auth-flow-complete`

Emitted by the Login Flow v2 polling task when the server delivers valid credentials.

**Payload**
```typescript
{ account: AccountDto }
```

**UI behaviour**: Wizard stores `accountId` in state and auto-advances from step 3 to step 4.

---

### `adagio://auth-flow-expired`

Emitted when the 5-minute countdown expires without successful auth.

**Payload**
```typescript
{}
```

**UI behaviour**: Shows expiry indicator on step 3; Continue button replaced by "Try again"
which calls `begin_auth_flow` again.

---

## Frontend Bindings (additions to `tauri.ts`)

```typescript
// ── Onboarding ────────────────────────────────────────────────────────────────

export interface ServerProbeDto {
  reachable: boolean;
  maintenance: boolean;
  version: string;
  version_ok: boolean;
  e2ee_available: boolean;
  tls_valid: boolean;
  latency_ms: number;
  error: string | null;
}

export interface AuthFlowInitDto {
  display_code: string;
  login_url: string;
  qr_svg: string;
  expires_at: number;
}

export interface RemoteStatsDto {
  total_bytes: number;
  used_bytes: number;
  file_count: number | null;
}

export interface OnboardingPrefsInput {
  local_folder: string;
  vfs_enabled: boolean;
  pin_pinned_folders: boolean;
  smart_bandwidth: boolean;
  watch_external_edits: boolean;
}

export const probeServer = (serverUrl: string): Promise<ServerProbeDto> =>
  invoke('probe_server', { serverUrl });

export const beginAuthFlow = (serverUrl: string): Promise<AuthFlowInitDto> =>
  invoke('begin_auth_flow', { serverUrl });

export const pickFolder = (): Promise<string | null> =>
  invoke('pick_folder');

export const getAccountRemoteStats = (accountId: string): Promise<RemoteStatsDto> =>
  invoke('get_account_remote_stats', { accountId });

export const completeOnboarding = (
  accountId: string,
  prefs: OnboardingPrefsInput,
): Promise<PairDto> =>
  invoke('complete_onboarding', { accountId, prefs });

export const listenAuthFlowComplete = (
  cb: (payload: { account: AccountDto }) => void,
): Promise<UnlistenFn> =>
  listen<{ account: AccountDto }>('adagio://auth-flow-complete', (e) => cb(e.payload));

export const listenAuthFlowExpired = (
  cb: () => void,
): Promise<UnlistenFn> =>
  listen('adagio://auth-flow-expired', () => cb());
```
