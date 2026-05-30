# Feature Specification: Network Awareness

**Feature ID**: 010  
**Created**: 2026-05-30  
**Status**: Draft  
**Priority**: P1 (metered connection) → P5 (settings panel)

---

## Overview

Adagio currently transfers files at maximum (or configured) speed regardless of network type or power state. This causes unexpected data charges on metered connections and unnecessary battery drain on laptops. Network Awareness automatically adjusts sync behaviour — pausing, throttling, or continuing — when the device's connection type, power source, or Wi-Fi network changes, with no manual intervention required.

---

## Problem Statement

Users on mobile hotspots or capped data plans can unknowingly exhaust their data allowance during a large sync. Laptop users lose battery faster than necessary because Adagio continues full-speed transfers after unplugging. Users who connect to public or untrusted Wi-Fi networks have no way to prevent sensitive files from transferring without manually pausing sync.

---

## User Stories

### US1 — Pause or throttle on metered connection (P1)
As a user on a mobile hotspot or capped data plan, I want Adagio to respect my metered connection so that sync does not run up unexpected data charges.

**Acceptance Criteria:**

- GIVEN `on-metered` is set to `pause`  
  WHEN the device connects to a metered network  
  THEN all sync activity stops within 5 seconds  
  AND status reports `paused (metered)`

- GIVEN `on-metered` is set to `throttle` with a Kbps limit  
  WHEN the device is on a metered network  
  THEN transfer throughput does not exceed the configured limit

- GIVEN the device moves from metered to an unmetered network  
  THEN normal sync resumes within 5 seconds

### US2 — Pause or throttle on battery (P2)
As a laptop user, I want to reduce or stop sync when unplugged so that battery life is preserved.

**Acceptance Criteria:**

- GIVEN `on-battery` is set to `pause`  
  WHEN the device unplugs from AC power  
  THEN sync pauses within 5 seconds and resumes within 5 seconds when AC is restored

- GIVEN `on-battery` is set to `throttle` with a Kbps limit  
  WHEN on battery  
  THEN sync proceeds at the configured throttle rate

### US3 — Block sync on specific SSIDs (P3)
As a user who connects to public or guest Wi-Fi, I want sync to pause automatically on named networks so that sensitive files are not transferred on untrusted connections.

**Acceptance Criteria:**

- GIVEN an SSID is in the block list  
  WHEN the device connects to that SSID  
  THEN sync pauses within 5 seconds  
  AND status reports `paused (blocked SSID)`

- GIVEN the device disconnects from a blocked SSID  
  THEN sync resumes within 5 seconds

### US4 — View current network state (P4)
As a user troubleshooting why sync is not running, I want a single status view showing the detected network conditions and the effective sync action.

**Acceptance Criteria:**

- Running the network status command shows: current SSID (or "unknown"), metered yes/no, power source (AC/battery), and the effective sync action (running / throttled / paused) with the reason
- The status reflects the current live state, not a cached snapshot

### US5 — Configure from the Settings panel (P5)
As a user who prefers not to use the terminal, I want to configure network awareness rules from the desktop app without opening a terminal.

**Acceptance Criteria:**

- Settings → Sync → Network shows controls for metered policy, battery policy, throttle Kbps, and the SSID block list
- Saving a rule persists it immediately and takes effect without restarting the app
- The panel shows the same live network state as the CLI status command, refreshing every 3 seconds

---

## Functional Requirements

### FR-1: Metered connection detection and policy enforcement
- The system detects whether the active network connection is metered
- Users configure one of three actions for metered connections: `allow` (default), `throttle`, or `pause`
- When `throttle` is selected, a Kbps limit must be set; it is shared with the battery throttle setting

### FR-2: Battery/power state detection and policy enforcement
- The system detects whether the device is running on AC power or battery
- Users configure one of three actions for battery state: `allow` (default), `throttle`, or `pause`
- Detection degrades gracefully: if the system API is unavailable, the condition is treated as `allow`

### FR-3: SSID block list
- Users add and remove Wi-Fi network names (SSIDs) from a block list
- When connected to a blocked SSID, sync pauses regardless of other policy settings
- SSID matching is exact and case-sensitive
- SSID detection degrades gracefully: if unavailable, no SSIDs are blocked

### FR-4: Policy priority and composition
- If multiple conditions apply simultaneously, the most restrictive action wins: `pause` > `throttle` > `allow`
- A blocked SSID always results in `pause`, overriding all other policies
- When all active conditions resolve to `allow`, sync runs at any previously configured bandwidth limit (or unlimited if none is set)

### FR-5: Reactive state changes
- The daemon monitors conditions continuously and reacts within 5 seconds of a state change
- When conditions improve, sync resumes automatically without user action
- In-progress transfers complete their current 64 KB chunk before stopping on a pause transition (clean stop)

### FR-6: CLI interface
- `adagio network status` — display current conditions and effective sync action
- `adagio network set [--on-metered allow|throttle|pause] [--on-battery allow|throttle|pause] [--throttle-kbps N]` — configure policy
- `adagio network block-ssid <ssid>` — add SSID to block list
- `adagio network unblock-ssid <ssid>` — remove SSID from block list
- `adagio network list-blocked` — list all blocked SSIDs
- All commands exit 0 on success; non-zero on daemon communication error

### FR-7: Configuration persistence
- Network policy is stored in the daemon's configuration file as a single top-level `network_policy` object
- The policy is global (not per-account)
- Default policy: `allow` for both metered and battery, empty SSID block list
- Configuration survives daemon restarts

### FR-8: Settings UI
- Settings → Sync includes a "Network" subsection
- Controls: metered action selector, battery action selector, throttle Kbps input, SSID block list (add/remove)
- Live status panel: current SSID, metered flag, power source, effective action + reason — refreshes every 3 seconds
- Saving rules applies them immediately without requiring a restart

---

## Success Criteria

1. **Data protection**: A user with `on-metered: pause` configured never triggers a file transfer while on a metered connection — zero bytes transferred
2. **Battery response time**: Sync ceases within 5 seconds of unplugging when `on-battery: pause` is set; resumes within 5 seconds of reconnecting to AC
3. **SSID blocking response time**: Sync pauses within 5 seconds of connecting to a blocked SSID on any supported platform
4. **Graceful degradation**: When platform detection APIs are unavailable, sync continues normally — no errors, no crashes, no false pauses
5. **Policy composition**: The most restrictive active policy always wins; no combination of conditions allows a transfer when `pause` is the effective action
6. **Persistence**: Network policy survives daemon restarts without reconfiguration
7. **CLI completeness**: All five `adagio network` subcommands are available and produce machine-readable JSON output when `--json` is passed

---

## Key Entities

### NetworkPolicy (persisted in config.json)
- `on_metered`: enum — `allow` | `throttle` | `pause` — default `allow`
- `on_battery`: enum — `allow` | `throttle` | `pause` — default `allow`
- `throttle_kbps`: number — shared speed cap used when either metered or battery action is `throttle`; 0 means unlimited
- `blocked_ssids`: list of strings — exact SSID names; empty by default

### NetworkState (runtime, not persisted)
- `metered`: boolean — is the current connection metered?
- `on_battery`: boolean — is the device on battery power?
- `ssid`: optional string — current Wi-Fi SSID, or absent if unknown or not on Wi-Fi
- `effective_action`: enum — `allow` | `throttle` | `pause`
- `reason`: string — human-readable explanation, e.g. "metered connection", "blocked SSID: CoffeeShop-Guest", "on battery"

---

## Assumptions

1. SSID matching is exact and case-sensitive; no wildcards in v1.
2. The `throttle_kbps` value is shared between metered and battery throttle actions. Separate rates per condition are a future enhancement.
3. Desktop machines without a battery report `on_battery: false`; the battery policy never activates on them.
4. The daemon polls system state every 3 seconds for conditions that do not support push notifications.
5. When transitioning to pause, any in-progress transfer completes its current chunk before stopping (clean stop, not abrupt kill).
6. Network policy does not interact with manual pause/resume. If the user manually paused sync, restoring network conditions does not auto-resume.

---

## Out of Scope (v1)

- Per-account network policies
- Time-based scheduling (different rules at different hours)
- Traffic shaping at the OS/kernel level
- VPN detection
- Cellular vs Wi-Fi distinction beyond the metered flag
- Wildcard or regex SSID matching
- Separate throttle rates for metered vs battery conditions
