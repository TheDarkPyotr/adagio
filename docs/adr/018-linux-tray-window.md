# ADR-018: Linux Tray Window Architecture

**Status**: Accepted
**Date**: 2026-06-03
**Feature**: 015-linux-tray-support

---

## Context

The Adagio desktop app uses Tauri 2 and includes a `TrayPopover` React component for the system-tray popover UI. Three gaps prevented the tray from working on Linux:

1. **Missing tray window**: `tauri.conf.json` declared no `"tray"`-labeled webview window. The Rust click handler called `get_webview_window("tray")` which always returned `None`.
2. **Wrong tray icon**: `trayIcon.iconPath` pointed to the full-colour 512 px app icon instead of the 24 px monochrome asset required by Linux notification areas.
3. **Position logic gap**: The inline Y-position calculation in the click handler always subtracted the window height from the click Y, which works for top panels (GNOME/Ubuntu) but positions the popover off-screen for bottom taskbars (KDE Plasma default).

Additional Linux-specific concerns:
- The `libayatana-appindicator3` (or `libappindicator3`) runtime library must be declared as a DEB/RPM dependency for AppIndicator support.
- On GNOME, the top bar does not show AppIndicator icons without the `gnome-shell-extension-appindicator` extension — the app must not crash if no tray host is present.
- Under Wayland, some compositors do not expose cursor position in tray click events (returns `(0, 0)`), requiring a fallback position.
- Keyboard shortcut labels in the popover display `⌘` which is macOS-only; Linux users expect `Ctrl+`.

---

## Decision

### 1. Separate webview window for the tray popover

Add a `"tray"`-labeled window entry to `app.windows` in `tauri.conf.json`, loading `index.html?tray`. `App.tsx` already dispatches to `<TrayPopover />` when the `?tray` search param is present. Using a separate hidden window (rather than a single-window overlay) provides correct OS-level z-ordering and focus semantics with no extra Rust code.

Properties chosen: `visible: false`, `decorations: false`, `alwaysOnTop: true`, `resizable: false`, `skipTaskbar: true`. `shadow` and `focus` are **not** valid Tauri 2 cross-platform window config fields and are omitted; drop-shadow is handled by CSS `box-shadow` and focus by `set_focus()` at runtime.

### 2. 24 px monochrome PNG for the tray icon

Use `icons/png-mono/adagio-icon-mono-24.png` as `trayIcon.iconPath`. This is the correct asset for Linux notification areas. `iconAsTemplate: true` is retained (macOS-only, harmless on Linux). The OS scales from the 24 px source for 16/22 px display sizes.

### 3. Panel-side heuristic for popover Y position

Extract `compute_popover_position(click, win_size, screen_w, screen_h) -> (i32, i32)` as a pure function in `lifecycle.rs`. The heuristic: if `click.y < screen_h / 2`, position the popover **below** the click point (top panel); otherwise position it **above** (bottom taskbar). When `click == (0.0, 0.0)` (Wayland compositor that does not expose cursor position), fall back to the top-right corner of the primary monitor at `(screen_w - 380 - 16, 48)`. The fallback is logged at `WARN` level.

### 4. No-crash guard for missing tray host

Log `tracing::warn!` when `app.tray_by_id("main")` returns `None`. The app continues without a tray icon. This covers vanilla GNOME without the AppIndicator extension and Wayland compositors without StatusNotifierItem support.

### 5. `get_platform()` command for keyboard shortcut labels

Add a minimal `#[tauri::command] pub fn get_platform() -> &'static str` using `#[cfg(target_os)]`. `TrayPopover.tsx` reads it on mount and renders `Ctrl+` (Linux/Windows) or `⌘` (macOS) as the modifier prefix in action-row keyboard hints.

### 6. Desktop-app autostart deferred to v2

The daemon already writes `~/.config/autostart/adagio-daemon.desktop` via `set_start_at_login`. A separate `adagio.desktop` entry for the GUI app would require `tauri-plugin-autostart` or direct file I/O. This is out of scope for v1; users launch the GUI manually and the daemon is already running.

---

## Consequences

**Positive**:
- Tray popover works on Ubuntu/GNOME (with AppIndicator extension), KDE Plasma 5/6, and XFCE
- App starts without crashing on any Linux desktop, with or without a tray host
- Popover position is correct for both top-panel and bottom-taskbar layouts
- `compute_popover_position` is a pure function — fully unit-testable without a Tauri runtime
- Keyboard shortcut labels are correct on all three platforms

**Negative / Constraints**:
- GNOME users must install `gnome-shell-extension-appindicator` to see the icon (documented in quickstart.md and README)
- `libayatana-appindicator3-1` (or `libappindicator3-1`) must be installed; declared as a DEB/RPM dependency
- Wayland compositors that do not expose cursor position get a fixed fallback position — non-ideal but non-blocking
- Desktop-app GUI autostart deferred; users must manually launch Adagio after login in v1
