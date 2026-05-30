# Data Model: Network Awareness (010)

## Entities

### NetworkPolicy (persisted in config.json)

```
NetworkPolicy {
    on_metered:    NetworkAction  // default: Allow
    on_battery:    NetworkAction  // default: Allow
    throttle_kbps: u64            // default: 0 (unlimited); shared by both throttle actions
    blocked_ssids: Vec<String>    // default: [] (empty); exact case-sensitive SSID names
}
```

**Validation rules:**
- `throttle_kbps` must be > 0 when `on_metered` or `on_battery` is `Throttle`; if 0 with Throttle, treat as Allow
- `blocked_ssids` entries are trimmed of leading/trailing whitespace before storage; duplicates are deduplicated (case-sensitive)

**Serialization:**
- Stored as top-level `network_policy` key in `config.json`
- `#[serde(default)]` on every field ensures backward compatibility
- Field names: `on_metered`, `on_battery`, `throttle_kbps`, `blocked_ssids`

---

### NetworkAction (enum)

```
NetworkAction {
    Allow,     // no restriction — sync runs at configured bandwidth limit
    Throttle,  // apply throttle_kbps limit
    Pause,     // halt all sync activity
}
```

**Serialization**: lowercase strings `"allow"` | `"throttle"` | `"pause"` in JSON/config.

---

### NetworkState (runtime only — never persisted)

```
NetworkState {
    metered:          bool           // is the active network connection metered?
    on_battery:       bool           // is the device on battery (not AC)?
    ssid:             Option<String> // current Wi-Fi SSID, or None if unknown
    effective_action: EffectiveAction
    reason:           String         // human-readable explanation
}
```

---

### EffectiveAction (computed from NetworkPolicy + NetworkState)

```
EffectiveAction {
    action:         NetworkAction
    throttle_kbps:  u64           // 0 if action is not Throttle
    reason:         String        // e.g. "metered connection", "on battery", "blocked SSID: CoffeeShop-Guest"
}
```

**Derivation rules (applied in order):**
1. If `ssid` is `Some(s)` and `s` in `blocked_ssids` → `Pause`, reason = `"blocked SSID: {s}"`
2. Else if `metered` and `on_metered` == `Pause` → `Pause`, reason = `"metered connection"`
3. Else if `on_battery` and `on_battery_action` == `Pause` → `Pause`, reason = `"on battery"`
4. Else if `metered` and `on_metered` == `Throttle` → `Throttle(throttle_kbps)`, reason = `"metered connection"`
5. Else if `on_battery` and `on_battery_action` == `Throttle` → `Throttle(throttle_kbps)`, reason = `"on battery"`
6. Else → `Allow`, reason = `""`

---

### NetworkStatusResponse (IPC response shape)

```json
{
  "metered": false,
  "on_battery": true,
  "ssid": "HomeNetwork",
  "effective_action": "throttle",
  "throttle_kbps": 200,
  "reason": "on battery",
  "policy": {
    "on_metered": "allow",
    "on_battery": "throttle",
    "throttle_kbps": 200,
    "blocked_ssids": ["CoffeeShop-Guest"]
  }
}
```

---

## State transitions

```
           connect to blocked SSID
 Allow  ──────────────────────────────────► Pause (SSID)
   ▲                                           │
   │ disconnect from blocked SSID              │
   └───────────────────────────────────────────┘

           on_metered=pause + metered=true
 Allow  ──────────────────────────────────► Pause (metered)
   ▲                                           │
   │ metered=false                             │
   └───────────────────────────────────────────┘

           on_battery=throttle + on_battery=true
 Allow  ──────────────────────────────────► Throttle
   ▲                                           │
   │ on_battery=false (AC restored)            │
   └───────────────────────────────────────────┘
```

**Notes:**
- Transitions to `Pause` always win over `Throttle` (policy composition rule).
- Manual pause/resume (via `DaemonRequest::PauseSyncAll`) is independent — it is not reflected in `NetworkState`. If the user manually paused, restoring network conditions does not auto-resume.
- State is checked every 3 seconds (polling) or on D-Bus signal (Linux event-driven).

---

## Config.json schema (delta)

```json
{
  "network_policy": {
    "on_metered": "allow",
    "on_battery": "allow",
    "throttle_kbps": 0,
    "blocked_ssids": []
  }
}
```

Missing `network_policy` key → use defaults (all `allow`, empty block list). Backward compatible.
