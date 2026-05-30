# Implementation Plan: Network Awareness

**Branch**: `010-network-awareness` | **Date**: 2026-05-30 | **Spec**: [spec.md](spec.md)

---

## Summary

Replace stub `is_metered_connection()` and `battery_level_percent()` in `adagio-core` with platform-specific real implementations, add SSID detection, introduce `NetworkPolicy` config type, build a `NetworkMonitor` background task that applies policy changes to the running sync engine, expose the monitor via IPC (3 new `DaemonRequest` variants), and surface controls in the CLI (`adagio network`) and the Settings panel (Settings → Sync → Network).

**Key ADR**: ADR-014 — `zbus` for Linux D-Bus (NetworkManager + UPower signals), `battery` crate for cross-platform power state, `wifi_scan` for SSID. Event-driven on Linux, 3-second polling fallback on macOS/Windows. Supersedes stubs in `cycle/mod.rs`.

---

## Technical Context

**Language/Version**: Rust stable (edition 2021, MSRV 1.83); TypeScript 5 + React for the UI layer

**Primary Dependencies** (new):
- `zbus` with `tokio` feature — Linux D-Bus (NetworkManager + UPower) — event-driven signal subscription
- `battery` crate — cross-platform AC/battery state (macOS IOKit, Windows WMI, Linux sysfs fallback)
- `wifi_scan` crate — cross-platform SSID detection (CoreWLAN / WlanApi / nl80211)
- `windows-rs` (existing, extend) — `INetworkListManager` for Windows metered detection
- No new IPC transport needed — existing NDJSON protocol extended

**Storage**: Existing `config.json` extended with top-level `network_policy` key (`#[serde(default)]`)

**Testing**: `cargo test`, `vitest` (UI). Mock `NetworkDetector` trait for unit tests; no real D-Bus in CI.

**Target Platform**: Linux (primary), macOS, Windows — all three CI targets must pass

**Performance Goals**: Monitor task idle CPU ~0% (event-driven); state check latency < 100 ms; reaction to state change < 5 s

**Constraints**: No `println!` in production; all new Rust public items need `///` docs; graceful degradation if platform APIs unavailable

**Scale/Scope**: Single global `NetworkPolicy`; no per-account policies

---

## Constitution Check

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ planned |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ enforced |
| ADR-014 recorded in `docs/adr/` | II. Documentation as Code | ✅ planned |
| Structured logging added to monitor task state changes | III. Observability | ✅ planned |
| No `println!` in production code paths | III. Observability | ✅ enforced |
| `NetworkMonitor` and `NetworkDetector` in `adagio-core/src/network/` — no direct coupling to daemon | IV. Extensibility | ✅ planned |
| Daemon calls monitor via `Arc<NetworkMonitor>` + trait boundary | IV. Extensibility | ✅ planned |
| Idle memory: monitor task adds ~0 MB RSS | V. Performance-Oriented | ✅ event-driven |
| UI actions (save policy) provide feedback within 100 ms | V. Performance-Oriented | ✅ local IPC |
| No new benchmarks required (no hot-path changes) | V. Performance-Oriented | N/A |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ enforced |
| `cargo fmt --check` passes | Dev Workflow | ✅ enforced |
| No new `unsafe` blocks (all platform APIs via safe crate bindings) | Dev Workflow | ✅ planned |
| All three platform CI targets (Linux, macOS, Windows) pass | Technology | ✅ `cfg` guards |

---

## Architecture

```
adagio-core/src/network/
├── mod.rs          — NetworkPolicy, NetworkState, EffectiveAction, NetworkAction
├── detector.rs     — NetworkDetector trait + platform impls (Linux/macOS/Windows)
├── monitor.rs      — NetworkMonitor task (holds Arc<DefaultSyncEngine>)
└── linux.rs        — zbus D-Bus implementation (cfg(target_os="linux"))
    macos.rs        — CoreWLAN + battery (cfg(target_os="macos"))
    windows.rs      — windows-rs + wifi_scan (cfg(windows))
    fallback.rs     — safe no-op detector for unsupported platforms

adagio-daemon/src/
└── network_monitor.rs  — spawn_network_monitor() — wires monitor into TaskTracker

adagio-ipc/src/types.rs
└── + GetNetworkStatus, SetNetworkPolicy, ManageBlockedSsid

adagio-daemon/src/dispatcher.rs
└── + handlers for three new variants

adagio-cli/src/
├── cli.rs              — + Commands::Network + NetworkCommand
└── handlers/network.rs — run_network_status/set/block/unblock/list

adagio-desktop/src/commands/network.rs
└── Tauri commands: get_network_status, set_network_policy, add/remove/list blocked SSIDs

adagio-desktop/src-ui/src/components/SettingsScene.tsx
└── + Network subsection in Sync tab
```

---

## File Structure

| File | Action |
|------|--------|
| `crates/adagio-core/src/network/mod.rs` | CREATE — types + public API |
| `crates/adagio-core/src/network/detector.rs` | CREATE — `NetworkDetector` trait + `DetectedState` |
| `crates/adagio-core/src/network/monitor.rs` | CREATE — `NetworkMonitor` task |
| `crates/adagio-core/src/network/linux.rs` | CREATE — zbus impl (Linux) |
| `crates/adagio-core/src/network/macos.rs` | CREATE — battery + wifi_scan (macOS) |
| `crates/adagio-core/src/network/windows.rs` | CREATE — windows-rs + wifi_scan (Windows) |
| `crates/adagio-core/src/network/fallback.rs` | CREATE — always-allow impl |
| `crates/adagio-core/src/lib.rs` | MODIFY — expose `pub mod network` |
| `crates/adagio-core/src/cycle/mod.rs` | MODIFY — remove/replace stubs with `NetworkMonitor` integration |
| `crates/adagio-core/Cargo.toml` | MODIFY — add `zbus`, `battery`, `wifi_scan` deps |
| `crates/adagio-daemon/src/network_monitor.rs` | CREATE — `spawn_network_monitor()` |
| `crates/adagio-daemon/src/main.rs` | MODIFY — spawn monitor in TaskTracker |
| `crates/adagio-daemon/src/dispatcher.rs` | MODIFY — 3 new IPC variant handlers |
| `crates/adagio-ipc/src/types.rs` | MODIFY — 3 new DaemonRequest variants |
| `crates/adagio-cli/src/cli.rs` | MODIFY — + `Commands::Network` + `NetworkCommand` |
| `crates/adagio-cli/src/handlers/network.rs` | CREATE — CLI handler functions |
| `crates/adagio-cli/src/handlers/mod.rs` | MODIFY — `pub mod network` |
| `crates/adagio-cli/src/run.rs` | MODIFY — dispatch `Commands::Network` |
| `crates/adagio-desktop/src/commands/network.rs` | CREATE — 5 Tauri command handlers |
| `crates/adagio-desktop/src/commands/mod.rs` | MODIFY — `pub mod network` |
| `crates/adagio-desktop/src/lib.rs` | MODIFY — register 5 new Tauri commands |
| `crates/adagio-desktop/src/config/mod.rs` | MODIFY — add `network_policy` to `SavedConfig` |
| `crates/adagio-desktop/src-ui/src/tauri.ts` | MODIFY — new bindings + types |
| `crates/adagio-desktop/src-ui/src/components/SettingsScene.tsx` | MODIFY — Network subsection |
| `crates/adagio-desktop/src-ui/src/__tests__/SettingsScene.test.tsx` | MODIFY — network UI tests |
| `crates/adagio-desktop/src-ui/src/__tests__/mocks/tauri.ts` | MODIFY — mock new commands |
| `docs/adr/014-network-awareness-detection.md` | CREATE — ADR-014 |

---

## Key Design Decisions

### NetworkDetector trait
```rust
/// Platform-specific network and power state probe.
pub trait NetworkDetector: Send + Sync {
    /// Returns true if the active connection is metered.
    fn is_metered(&self) -> bool;
    /// Returns true if the device is on battery (not AC).
    fn is_on_battery(&self) -> bool;
    /// Returns the current Wi-Fi SSID, or None if unknown.
    fn current_ssid(&self) -> Option<String>;
}
```
Test implementations use `MockNetworkDetector` with injectable state.

### NetworkMonitor task
- Holds `Arc<dyn NetworkDetector>`, `Arc<NetworkPolicy>` (RwLock-wrapped), `Arc<DefaultSyncEngine>`
- Runs a loop: check state → compare to previous → if changed, call engine methods
- Linux: subscribes to NM `StateChanged` D-Bus signal; falls back to 3s polling on signal error
- All other platforms: 3s `tokio::time::interval` poll
- On `effective_action` change:
  - `Pause` → `engine.pause().await`
  - `Throttle(kbps)` → `engine.update_bandwidth_limits(account_id, kbps, kbps).await` for all accounts, then resume if paused
  - `Allow` → clear bandwidth override + `engine.resume().await`

### Policy update path
`SetNetworkPolicy` IPC → dispatcher updates `NetworkPolicy` in `DaemonProcess` + persists to `config.json` → sends to `NetworkMonitor` via `Arc<RwLock<NetworkPolicy>>` shared reference (no restart needed)

### Interaction with manual pause
- `NetworkMonitor` only calls `pause()`/`resume()` on its own state transitions — it does NOT override a user-initiated manual pause
- Implementation: monitor tracks a `NetworkMonitor` flag; if user manually paused, monitor skips the `resume()` call even when conditions improve

---

## Dependency Notes

- `zbus` version 4.x (latest stable, async, no C deps) — add under `[target.'cfg(target_os = "linux")'.dependencies]`
- `battery` version 0.7.x — cross-platform, add under `[dependencies]`
- `wifi_scan` version 0.4.x — add under `[dependencies]`; on macOS this requires entitlements for Location Services; on headless Linux `nl80211` may be absent → degrade gracefully
- `windows-rs` already in workspace (for `INetworkListManager`) — extend existing dep

---

## Shell Commands

```bash
# Run all Rust tests
cargo test --workspace

# Run UI tests
cd crates/adagio-desktop/src-ui && npm test

# Check formatting
cargo fmt --all -- --check

# Lint
cargo clippy --workspace -- -D warnings

# Build daemon for manual testing
cargo build -p adagio-daemon -p adagio-cli
```
