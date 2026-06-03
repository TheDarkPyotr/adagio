# Contract: Tray Window IPC

**Feature**: 015-linux-tray-support
**Date**: 2026-06-03

The tray window is a secondary Tauri webview window (`label = "tray"`) that loads `index.html?tray`. It communicates with the Rust backend via the same Tauri IPC commands used by the main window.

---

## Commands Consumed by TrayPopover

All commands are already registered in `lib.rs`'s `invoke_handler`. No new registrations required except `get_platform`.

| Command | Rust handler | Input | Output | Used for |
|---------|-------------|-------|--------|----------|
| `get_status` | `commands::sync::get_status` | — | `SyncStatusDto` | Status headline, dot color |
| `get_activity_log` | `commands::sync::get_activity_log` | `{ limit: 3, filter: null }` | `ActivityEntryDto[]` | Recent section |
| `pause_sync` | `commands::sync::pause_sync` | — | `void` | "Pause syncing" action |
| `resume_sync` | `commands::sync::resume_sync` | — | `void` | "Resume syncing" action |
| `list_pairs` | `commands::pair::list_pairs` | — | `PairDto[]` | `local_root` for "Open folder" |
| `list_accounts` | `commands::account::list_accounts` | — | `AccountDto[]` | `server_url` for "Open in browser" |
| `get_platform` | `commands::mod::get_platform` | — | `"linux" \| "macos" \| "windows"` | Modifier key label |

---

## New Command: `get_platform`

**Rust signature**:
```rust
#[tauri::command]
pub fn get_platform() -> &'static str
```

**Return values**:
- `"linux"` — `#[cfg(target_os = "linux")]`
- `"macos"` — `#[cfg(target_os = "macos")]`
- `"windows"` — `#[cfg(target_os = "windows")]`

**TypeScript binding** (add to `tauri.ts`):
```typescript
export const getPlatform = (): Promise<'linux' | 'macos' | 'windows'> =>
  invoke('get_platform');
```

---

## Tray Window Lifecycle

```
App start
  └─ Rust lib.rs setup()
       ├─ Register TrayIcon (id="main")
       ├─ Create "tray" WebviewWindow (hidden, alwaysOnTop)
       └─ Wire on_tray_icon_event → toggle tray window

Left-click on tray icon
  ├─ if tray window visible → hide()
  └─ else
       ├─ Compute position (panel-side heuristic)
       ├─ set_position(x, y)
       ├─ show()
       └─ set_focus()

Tray window loses focus
  └─ WindowEvent::Focused(false) → hide()

"Quit Adagio" clicked
  └─ TrayPopover getAllWebviewWindows() → close all
```

---

## Events Received by Tray Window

The tray window is a full webview; it receives all broadcast Tauri events. Only the following are relevant to `TrayPopover`:

| Event | Payload | Effect |
|-------|---------|--------|
| `adagio://sync-status-changed` | `{ status, pair_id }` | Triggers `getStatus()` refresh |
| `adagio://daemon-connection-state` | `{ state }` | Updates status display if daemon disconnects |

`TrayPopover.tsx` currently polls on a 3 s interval via `setInterval`. Reactive event handling may be added in a future iteration to reduce latency.
