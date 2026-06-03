# IPC Contract: Bandwidth Throttling

**Feature**: 009-bandwidth-throttling
**Transport**: existing NDJSON IPC (same as feature 007)

---

## New DaemonRequest methods

### `get_bandwidth_status`

```json
→ {"id":1,"method":"get_bandwidth_status","params":{"account_id":null}}
← {"id":1,"result":{
     "upload_limit_kbps":500,
     "download_limit_kbps":0,
     "upload_bytes_per_sec":48200,
     "download_bytes_per_sec":0
   }}
```

`account_id: null` → applies to first account.  
`upload_limit_kbps: 0` → unlimited.  
`upload_bytes_per_sec` → rolling 10-second average; 0 if idle.

---

### `set_bandwidth_limits`

```json
→ {"id":2,"method":"set_bandwidth_limits","params":{
     "account_id":null,
     "upload_kbps":200,
     "download_kbps":0
   }}
← {"id":2,"result":{}}
```

`upload_kbps: 0` → remove upload limit.  
The limit takes effect within 1 second (at the next 64 KB chunk boundary).

---

### `clear_bandwidth_limits`

```json
→ {"id":3,"method":"clear_bandwidth_limits","params":{"account_id":null}}
← {"id":3,"result":{}}
```

Equivalent to `set_bandwidth_limits` with both fields set to 0.

---

## CLI contract additions

### `adagio bandwidth status [--json]`

**Human**:
```
DIRECTION   LIMIT         CURRENT SPEED
---------   -----------   -------------
Upload      500 Kbps      47.1 KB/s
Download    Unlimited     0 KB/s
```

**JSON**:
```json
{
  "upload_limit_kbps": 500,
  "download_limit_kbps": 0,
  "upload_bytes_per_sec": 48200,
  "download_bytes_per_sec": 0
}
```

### `adagio bandwidth set [--upload-kbps N] [--download-kbps N] [--json]`

At least one flag required. Sets only the specified direction(s).

**Human**: `Upload limit set to 200 Kbps`  
**JSON**: `{"ok": true, "upload_kbps": 200, "download_kbps": 0}`

### `adagio bandwidth clear [--json]`

**Human**: `Bandwidth limits cleared`  
**JSON**: `{"ok": true}`

---

## Desktop Settings UI contract

**Route**: Settings → Sync → Bandwidth (new sub-section inside the existing Sync panel)

**Fields**:
- "Upload limit" text input (Kbps, or blank for unlimited)
- "Download limit" text input (Kbps, or blank for unlimited)
- Save button
- Live display: "Current upload: X KB/s" and "Current download: X KB/s" (updates every 2s)

**Validation**: non-numeric or negative → show red border + error text; do not save.

**On save**: calls `set_bandwidth_limits` via the existing Tauri command layer.
