# Research: Network Awareness (010)

## 1. Platform detection APIs

### Decision: `zbus` for Linux D-Bus (NetworkManager + UPower)
- **Rationale**: Pure-Rust async D-Bus library with native Tokio support; no C `libdbus` dependency. `dbus-rs` requires C bindings and an async adapter shim — zbus is the idiomatic choice for Tokio daemons.
- **Alternatives considered**: `dbus-rs` (C FFI, heavier), subprocess calls to `nmcli`/`iwgetid` (fragile, parsing-dependent).
- **Linux property paths**:
  - Metered: `org.freedesktop.NetworkManager` → active connection's device → `org.freedesktop.NetworkManager.Device` property `Metered` (uint32: 0=unknown, 1=no, 2=yes, 3=yes-guessed). Treat 0 and 1 as unmetered.
  - Battery: `org.freedesktop.UPower` → `/org/freedesktop/UPower/devices/DisplayDevice` → property `State` (uint32: 1=charging, 2=discharging, 4=fully-charged, 5=pending-charge). On-battery = State 2.
  - SSID: NM `org.freedesktop.NetworkManager.Device.Wireless` → `ActiveAccessPoint` → `org.freedesktop.NetworkManager.AccessPoint` property `Ssid` (byte array → UTF-8).

### Decision: `battery` crate for cross-platform power state (macOS + Windows + Linux fallback)
- **Rationale**: Mature cross-platform wrapper over IOKit (macOS), WMI (Windows), sysfs (Linux). Zero unsafe FFI surface for the caller. Works on all three CI targets.
- **Alternatives considered**: Platform-specific raw bindings (more unsafe, maintenance burden), sysfs-only (Linux-only, already partially implemented).
- **Note**: The existing `battery_level_percent()` via sysfs remains as a fast Linux primary; `battery` crate is added as a cross-platform fallback for macOS/Windows.

### Decision: `wifi_scan` crate for SSID detection (all platforms)
- **Rationale**: Single crate covering CoreWLAN (macOS), WlanApi (Windows), and nl80211/iw (Linux). Avoids per-platform unsafe blocks.
- **Alternatives considered**: `corewlan-sys` (macOS-only), subprocess `iwgetid` (Linux-only, fragile), `WlanQueryInterface` direct (Windows-only). A unified crate reduces maintenance.
- **macOS caveat**: macOS 11+ requires Location Services to be enabled for SSID to be returned. Graceful degradation: if SSID is unavailable, no SSID-based blocking (treat as no blocked SSID match).

### Decision: `windows-rs` for metered detection on Windows
- **Rationale**: `INetworkListManager::GetNetworkConnections()` + `INetworkConnectionCost::GetCost()` exposes the `NLM_CONNECTION_COST_*` flags. `windows-rs` provides safe Rust bindings to WinRT with no manual unsafe.
- **Alternatives considered**: WMI (indirect, polling-only), subprocess `Get-NetConnectionProfile` (fragile PowerShell).

### Decision: Heuristic fallback for macOS metered detection
- **Rationale**: macOS does not expose a public "metered" API (it is a private `SCNetworkReachability` extension). On macOS, treat `on_metered` policy as if the network is never metered unless user explicitly marks it (manual override via CLI/UI flag future enhancement).
- **Alternatives considered**: `objc2` bindings to private SCNetworkReachability flags (fragile across OS versions, App Store rejection risk).

---

## 2. Monitoring architecture

### Decision: Event-driven (zbus signals) on Linux; 3-second polling on macOS/Windows
- **Rationale**: zbus provides async signal streams for NetworkManager `StateChanged` and UPower `Changed` events — zero latency, zero CPU cost between events. macOS/Windows lack equivalent async notification APIs accessible from Rust without significant FFI; 3-second polling meets the 5-second reaction requirement (worst case: 3s poll + engine reaction = 4s).
- **Alternatives considered**: Uniform polling (simple, but 3s wasted CPU on Linux where events are free), OS-specific watcher threads (complex, hard to cancel cleanly).

### Decision: Separate `NetworkMonitor` task in `crates/adagio-core/src/network/`
- **Rationale**: Network monitoring is an independent concern. Keeping it in `adagio-core` makes it testable without the full daemon stack and reusable if a future CLI or GUI needs it. The daemon spawns it via the existing `TaskTracker` pattern.
- **Alternatives considered**: Inline in `adagio-daemon/src/main.rs` (couples core logic to binary), new crate `adagio-network` (unnecessary crate proliferation for this scope).

### Decision: Build on existing `SyncConfig.pause_on_metered` / `min_battery_percent` foundation
- **Rationale**: `cycle/mod.rs` already has `is_metered_connection()` and `battery_level_percent()` stubs plus `should_suspend()`. The new feature replaces these stubs with real implementations and adds SSID + policy-driven throttle/pause. This avoids a parallel system.
- **Migration**: `SyncConfig { pause_on_metered, min_battery_percent }` is superseded by `NetworkPolicy { on_metered, on_battery, throttle_kbps, blocked_ssids }`.

### Decision: State deduplication in NetworkMonitor
- **Rationale**: Avoid hammering `pause()` / `resume()` / `update_bandwidth_limits()` on every poll tick. The monitor caches the last `NetworkState` and only calls the engine when `effective_action` changes.

---

## 3. IPC and persistence

### Decision: Three new DaemonRequest variants (GetNetworkStatus, SetNetworkPolicy, ManageBlockedSsid)
- **Rationale**: Consistent with the existing IPC pattern (bandwidth throttling used the same approach). The daemon dispatcher handles persistence to `config.json` and applies the policy to the running monitor.
- **Alternatives considered**: Reusing existing bandwidth IPC (wrong abstraction boundary — network policy is distinct from user-configured bandwidth limits).

### Decision: NetworkPolicy stored as top-level `network_policy` in config.json with `#[serde(default)]`
- **Rationale**: `#[serde(default)]` ensures backward compatibility with existing config files that don't have the field. Default is `allow` for all conditions, empty SSID list — no change in behaviour for users who never configure the feature.

---

## 4. ADR to record

**ADR-014**: Use `zbus` (Linux) + polling fallback (macOS/Windows) for network/power state detection, `battery` crate for power state, `wifi_scan` for SSID — supersedes the `is_metered_connection()` and `battery_level_percent()` stubs in `cycle/mod.rs`.
