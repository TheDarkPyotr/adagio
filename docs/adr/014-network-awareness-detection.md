# ADR-014: Network and Power State Detection Strategy

**Status**: Accepted  
**Date**: 2026-05-30  
**Feature**: 010-network-awareness

---

## Context

Feature 010 (Network Awareness) requires the daemon to detect three runtime conditions and
react within 5 seconds:

1. **Metered connection** — is the active network interface on a metered plan?
2. **Battery/power state** — is the device on battery or AC power?
3. **SSID** — what Wi-Fi network is the device connected to?

Each condition is needed on three platform targets: Linux, macOS, Windows.

The existing codebase has stub implementations:
- `is_metered_connection()` in `crates/adagio-core/src/cycle/mod.rs` — always returns false
- `battery_level_percent()` — sysfs-only Linux implementation

This ADR decides the library choices, monitoring architecture, and graceful-degradation
strategy that supersede these stubs.

---

## Decision

### Library choices

| Condition | Linux | macOS | Windows |
|-----------|-------|-------|---------|
| Metered | `zbus` → NetworkManager `Device.Metered` property | No public API → always false | `windows-rs` → `INetworkListManager` `GetCost()` |
| Battery | `zbus` → UPower `DisplayDevice.State`, fallback to sysfs | `battery` crate (IOKit) | `battery` crate (WMI) |
| SSID | `zbus` → NM `AccessPoint.Ssid` | `wifi_scan` crate (CoreWLAN) | `wifi_scan` crate (WlanApi) |

**`zbus` v4** (pure Rust, native Tokio async) is chosen over `dbus-rs` (requires C `libdbus`,
needs a thread-based async adapter). All D-Bus calls are wrapped in `catch_unwind`-equivalent
error handling: any `zbus::Error` degrades to the safe default (false / None).

**`battery` v0.7** provides a cross-platform, safe API over IOKit (macOS) and WMI (Windows)
for power state. It replaces direct sysfs parsing on macOS/Windows; the Linux sysfs fallback
in `battery_level_percent()` is retained as a secondary fallback when UPower D-Bus is absent.

**`wifi_scan` v0.4** covers all three platforms for SSID detection. On macOS 11+, Location
Services must be enabled for SSID to be returned; if not, `current_ssid()` returns `None`
and no SSID-based blocking occurs (graceful degradation, not an error).

### Monitoring architecture

A `NetworkMonitor` task is spawned by the daemon alongside the IPC server and runs in the
`TaskTracker` for clean shutdown. It holds:
- `Arc<dyn NetworkDetector>` — platform-specific probe
- `Arc<RwLock<NetworkPolicy>>` — shared with the dispatcher (policy updates take effect within one poll tick)
- `Arc<DefaultSyncEngine>` — to call `pause()`, `resume()`, `update_bandwidth_limits()`

On Linux, the monitor attempts to subscribe to NetworkManager `StateChanged` D-Bus signals
for zero-latency reactions; if signal subscription fails, it falls back to the 3-second poll.
On macOS and Windows, 3-second polling is used exclusively. The 3-second interval satisfies
the 5-second reaction requirement (worst case: poll miss + engine reaction = ~4 s).

State changes are deduplicated: the monitor caches the last `EffectiveAction` and only calls
the engine when the action changes.

### Interaction with manual pause/resume

`NetworkMonitor` tracks a `user_paused: bool` flag (updated when the dispatcher handles
`PauseSyncAll` / `ResumeSyncAll`). When `user_paused` is true, the monitor never calls
`engine.resume()` even when network conditions improve. This ensures network policy does not
override deliberate user actions.

---

## Alternatives Considered

| Alternative | Rejected because |
|-------------|-----------------|
| `dbus-rs` for Linux D-Bus | Requires C `libdbus`; async adapter adds thread overhead |
| Subprocess `nmcli` / `iwgetid` | Fragile string parsing; process spawn latency; not testable |
| Uniform 3-second polling on all platforms | Wastes CPU on Linux where D-Bus events are free |
| `objc2` bindings for macOS metered detection | macOS metered flag is a private SCNetworkReachability extension; breaks across OS versions |
| Separate `adagio-network` crate | Unnecessary crate proliferation; module in `adagio-core` is sufficient |

---

## Consequences

- `crates/adagio-core/Cargo.toml` gains `zbus`, `battery`, `wifi_scan` dependencies
- `is_metered_connection()` and `battery_level_percent()` stubs in `cycle/mod.rs` are removed
- `SyncConfig.pause_on_metered` and `SyncConfig.min_battery_percent` are removed (superseded by `NetworkPolicy`)
- All three conditions degrade gracefully to "no restriction" when platform APIs are unavailable
- macOS users cannot get metered detection until Apple exposes a public API (documented in UI as "Not available on macOS")
