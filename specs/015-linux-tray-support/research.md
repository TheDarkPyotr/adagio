# Research: Linux System Tray Support

**Feature**: 015-linux-tray-support
**Phase**: 0 — Pre-design research
**Date**: 2026-06-03

---

## Finding 1: Tauri 2 Tray Window — Critical Missing Piece

**Decision**: Add a `"tray"`-labeled webview window to `tauri.conf.json`.

**Rationale**: `lib.rs` calls `app_handle.get_webview_window("tray")` but the current `tauri.conf.json` only declares one unnamed window (label defaults to `"main"`). The tray click handler always returns `None` from `get_webview_window` and silently no-ops. This is the root cause of the tray popover not appearing.

**Implementation**:
```json
{
  "label": "tray",
  "url": "index.html?tray",
  "width": 380,
  "height": 520,
  "visible": false,
  "decorations": false,
  "alwaysOnTop": true,
  "resizable": false,
  "skipTaskbar": true,
  "focus": false,
  "shadow": false
}
```

`App.tsx` already dispatches via `const IS_TRAY = new URLSearchParams(window.location.search).has('tray')` and renders `<TrayPopover />` when the flag is set. No React changes needed for routing.

**Alternatives considered**:
- Dynamic window creation in Rust at click time — rejected: adds latency; first click would be slow.
- Single-window overlay — rejected: breaks focus semantics; popover must be a separate window to get OS-level z-ordering.

---

## Finding 2: Linux Tray Icon Protocol Stack

**Decision**: Rely on Tauri 2's built-in `tray-icon` crate with the `libayatana-appindicator3` backend; declare `libayatana-appindicator3-1` as a Debian runtime dependency.

**Rationale**: Tauri 2 uses the `tray-icon` crate which auto-selects:
- `libappindicator3` on Ubuntu ≤ 20.04 / Debian
- `libayatana-appindicator3` on Ubuntu 22.04+ (the Canonical-maintained fork)

On GNOME (the dominant Linux desktop), the system status bar does **not** show AppIndicator icons natively. Users must install `gnome-shell-extension-appindicator` (Ubuntu: `sudo apt install gnome-shell-extension-appindicator`) or the GNOME extension from extensions.gnome.org. KDE Plasma 5/6 and XFCE support AppIndicator natively.

The app must **not crash** if no indicator host is running — the `tray-icon` crate wraps the registration in a Result; we handle the error with a tracing warning.

**AppIndicator DEB dependency** (tauri.conf.json bundle.deb.depends):
```json
["libayatana-appindicator3-1 | libappindicator3-1", "libwebkit2gtk-4.1-0"]
```

**RPM equivalent** (tauri.conf.json bundle.rpm.depends):
```json
["libayatana-appindicator3"]
```

**Alternatives considered**:
- Pure XDG StatusNotifierItem over D-Bus without libappindicator — technically correct but complex to implement without the library; Tauri already handles this through tray-icon crate.

---

## Finding 3: Tray Icon Asset — `iconAsTemplate` vs Mono PNG

**Decision**: On Linux, use the pre-existing `icons/png-mono/adagio-icon-mono-24.png` as the tray icon. Keep `iconAsTemplate: true` in `tauri.conf.json` (it is harmless on Linux — ignored by the tray-icon crate).

**Rationale**: `iconAsTemplate: true` is a macOS-only flag. On Linux, the icon is rendered as a plain PNG. The current `tauri.conf.json` points to `"icons/icon.png"` which is the full-colour app icon at 512×512 — too large and colourful for notification areas. Linux notification areas expect 22–24 px monochrome PNGs.

The existing `icons/png-mono/adagio-icon-mono-24.png` is exactly the right asset. A platform-conditional tray icon path can be set in `tauri.conf.json`:

```json
"trayIcon": {
  "iconPath": "icons/png-mono/adagio-icon-mono-24.png",
  "iconAsTemplate": true,
  "id": "main"
}
```

This single 24 px icon works across Linux DEs. macOS will treat it as a template image (respects light/dark). Windows uses it as-is (also fine at 24 px).

**Alternatives considered**:
- Separate `trayIcon` config per platform via Tauri's `bundle.targets` — not needed; same path works.

---

## Finding 4: Tray Popover Positioning on Linux

**Decision**: Adjust the position logic in `lib.rs` to compute `y` based on whether the tray click is in the top or bottom half of the primary monitor.

**Rationale**: The current code computes:
```rust
let y = (position.y as i32) - (win_size.height as i32) - 8;
```
This places the popover **above** the click point, which is correct for a top panel (GNOME, Ubuntu). On KDE Plasma with a bottom taskbar, this positions the popover above the taskbar off-screen.

**Fix**: Detect panel side from click position:
```rust
// Get primary monitor height; if unavailable, assume top panel.
let screen_height = app_handle
    .primary_monitor()
    .ok()
    .flatten()
    .map(|m| m.size().height as i32)
    .unwrap_or(1080);

let is_top_panel = position.y as i32 < screen_height / 2;

let (x, y) = if is_top_panel {
    // Popover below icon (top bar)
    let x = (position.x as i32) - (win_size.width as i32 / 2);
    let y = position.y as i32 + 8;
    (x.max(0), y)
} else {
    // Popover above icon (bottom taskbar)
    let x = (position.x as i32) - (win_size.width as i32 / 2);
    let y = (position.y as i32) - (win_size.height as i32) - 8;
    (x.max(0), y.max(0))
};
```

**Wayland caveat**: Under Wayland, `position` in `TrayIconEvent::Click` may be `(0, 0)` on compositors that do not expose cursor position to client apps (GNOME Wayland). In this case, fall back to the top-right corner of the primary monitor: `x = screen_width - win_size.width - 16, y = 48`. Log the fallback at `WARN` level.

**Alternatives considered**:
- Query the panel orientation via D-Bus — adds significant complexity and GNOME/KDE-specific code; heuristic is simpler and covers >95% of real-world layouts.

---

## Finding 5: Focus-Loss Dismiss on Linux

**Decision**: Keep the existing `WindowEvent::Focused(false) → window.hide()` handler; add a `tauri::WindowEvent::CloseRequested` guard to prevent accidental full-window closure.

**Rationale**: The current `on_window_event` handler already covers the tray window:
```rust
tauri::WindowEvent::Focused(false) if window.label() == "tray" => {
    window.hide().ok();
}
```
Under X11, this fires reliably. Under Wayland (GNOME Shell), it may fire spuriously when a sub-surface (tooltip, notification) steals focus — but this causes at most an unexpected dismiss, not a crash. Document this limitation; do not attempt platform-specific workarounds in v1.

**Alternatives considered**:
- Global mouse-click listener via X11/Wayland API — complex, platform-specific, out of scope for v1.

---

## Finding 6: Keyboard Shortcut Labels (⌘ → Ctrl on Linux)

**Decision**: Expose a Tauri command `get_platform()` returning `"linux" | "macos" | "windows"` and use it in `TrayPopover.tsx` to render `Ctrl` vs `⌘`.

**Rationale**: The current tray actions show `⌘O`, `⌘B`, etc. — macOS-only labels. On Linux the convention is `Ctrl+O`. The shortcut labels are display-only (the action rows use `onClick`, not actual keyboard bindings), so this is a cosmetic change.

Simplest implementation: add to `commands/mod.rs`:
```rust
#[tauri::command]
pub fn get_platform() -> &'static str {
    #[cfg(target_os = "linux")]   { "linux" }
    #[cfg(target_os = "macos")]   { "macos" }
    #[cfg(target_os = "windows")] { "windows" }
}
```
Then in `TrayPopover.tsx`:
```tsx
const [platform, setPlatform] = useState<string>('macos');
useEffect(() => { getPlatform().then(setPlatform); }, []);
const mod = platform === 'macos' ? '⌘' : 'Ctrl+';
```

**Alternatives considered**:
- `navigator.platform` in the browser context — deprecated and unreliable in Tauri webview; use the Tauri command instead.
- `window.__TAURI__.os.platform()` — available but requires tauri-plugin-os; adding a lightweight command is simpler.

---

## Finding 7: "Start at Login" — Autostart Scope

**Decision**: The existing `set_start_at_login` command forwards to the daemon, which handles XDG autostart. No changes needed to the autostart mechanism for this feature.

**Rationale**: `daemon.rs` already has `set_start_at_login` which calls `DaemonRequest::SetStartAtLogin`. The daemon writes `~/.config/autostart/ai.neuralagent.adagio.desktop`. This launches the **daemon** at login, not the desktop app. For the tray icon to appear automatically, the desktop app also needs an autostart entry. This is a separate concern already covered by Tauri's `tauri-plugin-autostart` or the user launching Adagio from the application launcher.

For v1, the scope is: the tray icon appears when the user manually starts Adagio. The "Start at login" preference (for the daemon) is a separate pre-existing feature. A future feature can add GUI-launched autostart for the desktop app.

---

## Summary of Decisions

| # | Decision | Impact |
|---|----------|--------|
| 1 | Add `"tray"` window to tauri.conf.json pointing to `index.html?tray` | **Critical** — popover cannot appear without this |
| 2 | Use `libayatana-appindicator3` dependency; log warning if host unavailable | **Critical** — tray icon requires this on Linux |
| 3 | Use `icons/png-mono/adagio-icon-mono-24.png` for tray icon | Medium — visual quality |
| 4 | Panel-side heuristic for popover Y position; Wayland fallback to top-right | **Critical** — popover position correctness |
| 5 | Keep focus-loss handler as-is; document Wayland quirk | Low — acceptable for v1 |
| 6 | Add `get_platform()` command; update TrayPopover shortcut labels | Low — cosmetic |
| 7 | Autostart for desktop app is out of scope for this feature | Scope boundary |
