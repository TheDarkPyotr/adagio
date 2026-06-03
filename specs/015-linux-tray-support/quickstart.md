# Quickstart: Linux System Tray Support

**Feature**: 015-linux-tray-support
**Date**: 2026-06-03

---

## Prerequisites

> **"Start at login"** in Preferences controls **daemon** auto-start only (writes
> `~/.config/autostart/adagio-daemon.desktop`). The Adagio GUI app must be opened manually
> after login in v1. Full desktop-app autostart is planned for v2.

### Linux Runtime Dependencies

Install the AppIndicator library (required for the tray icon to appear on GNOME/KDE/XFCE):

```bash
# Ubuntu 22.04 / 24.04
sudo apt install libayatana-appindicator3-1

# Ubuntu 20.04 / Debian
sudo apt install libappindicator3-1

# Fedora
sudo dnf install libayatana-appindicator-gtk3

# Arch
sudo pacman -S libayatana-appindicator
```

### GNOME Users: AppIndicator Extension

GNOME's default panel does **not** show tray icons. Install the extension:

```bash
# Ubuntu (package)
sudo apt install gnome-shell-extension-appindicator
# Then enable it:
gnome-extensions enable appindicatorsupport@rgcjonas.gmail.com
# Log out and back in.
```

Or install from https://extensions.gnome.org/extension/615/appindicator-support/

KDE Plasma 5/6 and XFCE support the tray icon without any additional setup.

---

## Build Setup

```bash
# Install Tauri CLI (if not already present)
cargo install tauri-cli --version "^2"

# Install Node dependencies
cd crates/adagio-desktop/src-ui
npm install
cd ../../..

# Build and run in dev mode
cargo tauri dev --manifest-path crates/adagio-desktop/Cargo.toml
```

The app opens the main window. The tray icon should appear in your notification area (top bar on GNOME with the extension, system tray on KDE/XFCE).

---

## Testing the Tray

### Verify tray icon appears

```bash
# Start the app
cargo tauri dev --manifest-path crates/adagio-desktop/Cargo.toml &

# Check that the DBus indicator is registered
busctl --user monitor --match "interface='org.kde.StatusNotifierItem'"
# or
gdbus monitor --session --dest org.kde.StatusNotifierWatcher
```

### Open the tray popover

1. Click the tray icon in the notification area.
2. The popover should appear anchored below (top panel) or above (bottom panel) the icon.
3. Click outside the popover — it should dismiss.
4. Click the icon again — it should reappear.

### Test each action

| Action | Expected result |
|--------|-----------------|
| Open Adagio folder | File manager opens at sync root |
| Open in browser | Browser opens server URL |
| Pause syncing | Daemon pauses; label → "Resume syncing" |
| Preferences… | Main window raises to foreground |
| Quit Adagio | All windows close cleanly |

### Test status states

Start/stop the daemon to exercise all status states:

```bash
# Trigger syncing state (if daemon is running)
# The status headline should update within 5 seconds of the daemon state change

# Stop the daemon to trigger "unreachable"
pkill adagio-daemon
# Open popover → should show "Unreachable" in red
```

### Wayland testing

```bash
# Force Wayland session (if running X11)
WAYLAND_DISPLAY=wayland-0 GDK_BACKEND=wayland cargo tauri dev ...
# Expected: tray popover falls back to top-right corner if position is (0,0)
```

---

## Frontend Dev (TrayPopover only)

To develop the tray popover in isolation in the browser:

```bash
cd crates/adagio-desktop/src-ui
npm run dev
# Open: http://localhost:5173/?tray
```

The `IS_TRAY` flag in `App.tsx` renders `<TrayPopover />` directly.

---

## Running Tests

```bash
# Rust unit tests (includes tray position logic)
cargo test -p adagio-desktop

# Frontend unit tests
cd crates/adagio-desktop/src-ui
npm test
# TrayPopover.test.tsx covers status states, action labels, recent rows
```
