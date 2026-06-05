# Quickstart: Desktop App Lifecycle (Feature 002)

**Feature**: 002-desktop-app-lifecycle
**Updated**: 2026-05-24

This guide covers testing the persistent config, running sync engine, and file browser
features end-to-end in a local development environment.

---

## Prerequisites

All prerequisites from `specs/001-nextcloud-file-sync/quickstart.md` apply.
Additionally:

| Tool        | Version | Purpose                              |
|-------------|---------|--------------------------------------|
| Tauri CLI   | 2.x     | `cargo install tauri-cli --version "^2"` |
| Node.js     | ≥ 20    | Svelte dev server                    |
| npm         | ≥ 10    | Frontend dependencies                |

---

## Running the Desktop App

```bash
cd crates/adagio-desktop
npm install --prefix src-ui
cargo tauri dev
```

The app opens with a dev-reload server for the Svelte frontend. Config is saved to
the platform app-config directory:

| Platform | Path                                                           |
|----------|----------------------------------------------------------------|
| Linux    | `~/.config/adagio/config.json`                  |
| macOS    | `~/Library/Application Support/adagio/config.json` |
| Windows  | `%APPDATA%\adagio\config.json`                  |

---

## Testing Configuration Persistence

1. Launch the app with `cargo tauri dev`
2. Add a Nextcloud account (Settings → Add Account)
3. Create a sync pair (Pairs → New Pair)
4. Quit the app (`Ctrl-C` in the terminal or close the window)
5. Relaunch — the account and pair should be present without reconfiguration

**Verify**: `cat ~/.config/adagio/config.json` (Linux) should show
the account and pair entries.

---

## Testing Automatic Sync on Startup

Requires a running Nextcloud instance (see `specs/001-nextcloud-file-sync/quickstart.md`
for the Docker setup).

1. Set `RUST_LOG=adagio=debug` before launching
2. Create a sync pair pointing to the Docker Nextcloud
3. Add a file to the Nextcloud web UI
4. Quit and relaunch the app
5. Within 5 seconds, the file should appear in the local sync folder

Watch for log output like:
```
INFO adagio_core::cycle::runner: starting sync cycle pair_id=...
INFO adagio_core::cycle::propagator: download path=new_file.txt
```

---

## Testing the File Browser

1. Launch the app with a configured sync pair
2. Wait for the first sync cycle to complete (watch logs)
3. Navigate to the Dashboard → file browser panel
4. Files in the local sync folder should appear with status badges
5. Add a new file to the local sync folder; manually trigger sync (sync button)
6. The new file should appear with `pending_upload` status, then `synced`

---

## Running Unit Tests for This Feature

```bash
# Config persistence tests
cargo test -p adagio-core --lib config
cargo test -p adagio-desktop --lib

# Runner / engine tests
cargo test -p adagio-core --lib cycle::runner
```

---

## Inspecting the Config File

```bash
# Linux/macOS
cat ~/.config/adagio/config.json | python3 -m json.tool

# Reset to clean state (removes all saved accounts and pairs)
rm ~/.config/adagio/config.json
```

**Note**: Removing the config file does NOT remove credentials from the OS keychain.
Use the app's "Remove Account" action to clean up keychain entries properly.
