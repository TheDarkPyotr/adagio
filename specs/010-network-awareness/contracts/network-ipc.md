# IPC Contract: Network Awareness (010)

All new variants follow the existing NDJSON IPC protocol (ADR-009).
Requests are `{"method": "<snake_case>", "params": {...}}`.
Responses are `{"id": N, "result": <value>}` or `{"id": N, "error": "<msg>"}`.

---

## New DaemonRequest variants

### GetNetworkStatus
```json
{ "method": "get_network_status", "params": {} }
```
**Response** — `DaemonResponse::Status(Value)`:
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

### SetNetworkPolicy
```json
{
  "method": "set_network_policy",
  "params": {
    "on_metered": "pause",
    "on_battery": "throttle",
    "throttle_kbps": 200
  }
}
```
- All params are optional; omitted fields keep their current value
- `throttle_kbps` must be > 0 if `on_metered` or `on_battery` is `"throttle"`

**Response** — `DaemonResponse::Unit {}` on success.

---

### ManageBlockedSsid
```json
{
  "method": "manage_blocked_ssid",
  "params": {
    "action": "add",
    "ssid": "CoffeeShop-Guest"
  }
}
```
- `action`: `"add"` | `"remove"` | `"list"`
- `ssid`: required for `add` and `remove`; omitted for `list`

**Response**:
- `add` / `remove` → `DaemonResponse::Unit {}`
- `list` → `DaemonResponse::Strings(Vec<String>)` — the full blocked SSID list

---

## New Tauri commands

| Command | Params | Return |
|---------|--------|--------|
| `get_network_status` | none | `NetworkStatusDto` (JSON value) |
| `set_network_policy` | `onMetered?, onBattery?, throttleKbps?` | `void` |
| `add_blocked_ssid` | `ssid: string` | `void` |
| `remove_blocked_ssid` | `ssid: string` | `void` |
| `list_blocked_ssids` | none | `string[]` |

---

## CLI subcommand contract

```
adagio network status [--json]
adagio network set [--on-metered allow|throttle|pause]
                   [--on-battery allow|throttle|pause]
                   [--throttle-kbps N]
adagio network block-ssid <ssid>
adagio network unblock-ssid <ssid>
adagio network list-blocked [--json]
```

**Human output** (`network status`):
```
CONDITION       STATE           POLICY
Metered         No              Allow
Battery         Yes             Throttle (200 Kbps)
SSID            HomeNetwork     —

Effective action: Throttle · 200 Kbps (on battery)
```

**JSON output** (`network status --json`):
Full `NetworkStatusDto` JSON object (same as IPC response).

---

## TypeScript types (tauri.ts)

```typescript
export type NetworkAction = 'allow' | 'throttle' | 'pause';

export interface NetworkPolicyDto {
  on_metered: NetworkAction;
  on_battery: NetworkAction;
  throttle_kbps: number;
  blocked_ssids: string[];
}

export interface NetworkStatusDto {
  metered: boolean;
  on_battery: boolean;
  ssid: string | null;
  effective_action: NetworkAction;
  throttle_kbps: number;
  reason: string;
  policy: NetworkPolicyDto;
}
```

---

## Push event (optional, future)

A `DaemonEvent::NetworkStateChanged` push event may be added later to let the UI react immediately without polling. Not required for v1 (3-second poll in UI is sufficient).
