# ADR 004: Configuration Persistence Format and Location

**Date**: 2026-05-24
**Status**: Accepted
**Feature**: 002-desktop-app-lifecycle

## Context

The desktop app needs to persist account and sync-pair configuration across restarts.
The data to persist is small (typically under 10 KB), human-readable inspection is
desirable for debugging, and credentials must never be written to disk.

## Decision

Store configuration as a JSON file (`config.json`) in the platform's application config
directory, resolved by Tauri's `app_config_dir()`:

- **Linux**: `~/.config/adagio/config.json`
- **macOS**: `~/Library/Application Support/adagio/config.json`
- **Windows**: `%APPDATA%\adagio\config.json`

Serialisation via `serde_json` (already a workspace dependency).
The file contains account metadata and pair configuration.
Credentials (app-passwords, OAuth tokens) are stored exclusively in the OS keychain;
only the keychain lookup key is written to the config file.

## Consequences

**Positive**:
- Human-readable: developers can inspect and manually repair the file
- Zero new dependencies: `serde_json` already in use
- Platform-appropriate location via Tauri, no manual path logic
- Clean separation: config file = metadata; keychain = secrets

**Negative**:
- JSON offers no schema migration tooling; a `version` field is included in
  `SavedConfig` to support future migrations via match-on-version logic
- File-level atomicity (write temp + rename) needed to prevent corruption on crash

## Alternatives Considered

- **TOML**: More human-friendly but requires adding the `toml` crate; not justified for
  a file that is rarely hand-edited.
- **SQLite (same DB as journal)**: Mixes ephemeral sync state with persistent app config;
  harder to wipe one without wiping the other; overkill for a few KB of data.
- **Platform-native storage** (NSUserDefaults, Windows Registry): Non-portable and harder
  to inspect or migrate; Tauri's file-based approach is simpler and cross-platform.
