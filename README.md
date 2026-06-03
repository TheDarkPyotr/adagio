# adagio
![Adagio Banner](docs/adr/adagio_banner.png)

A high-performance, cross-platform Nextcloud Desktop sync client written in Rust.

---

## Overview

Adagio syncs files between your devices and a Nextcloud server. It runs as a background daemon (`adagio-daemon`) and exposes a native GUI (`adagio-desktop`, built with Tauri), a CLI (`adagio-cli`), and a virtual filesystem (FUSE3 on Linux, extensible to CfAPI/FileProvider). All communication between the GUI/CLI and the daemon happens over a Unix socket using NDJSON RPC.

**Key properties**

- Conflict-aware: three resolution policies (Ask, Keep-Local, Keep-Remote) with a wizard UI
- Bandwidth-aware: per-account upload/download limits with chunk-level token-bucket throttling
- Network-aware: respects metered connections, battery state, and SSID blocklists
- VFS: on-demand file delivery via FUSE3 (pin/evict for offline access)
- E2EE: client-side AES-128-GCM encryption with BIP-39 mnemonic pairing (feature 013)

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│  adagio-desktop (Tauri + React/TypeScript)           │
│  adagio-cli     (clap v4)                            │
│              │ NDJSON IPC (Unix socket)               │
│  adagio-daemon  (tokio, background process)           │
│    ├─ SyncEngine  (adagio-core)                      │
│    ├─ VfsPairRunner  (adagio-vfs / FUSE3)            │
│    └─ E2eeRunner  (adagio-e2ee)                      │
│              │ WebDAV / OCS API                       │
│  Nextcloud server                                     │
└──────────────────────────────────────────────────────┘
```

### Crates

| Crate | Description |
|-------|-------------|
| `adagio-core` | Sync engine, journal (SQLite WAL), transfer primitives, reconciler, propagator, bandwidth |
| `adagio-daemon` | Standalone daemon binary — IPC server, dispatcher, E2EE runner |
| `adagio-desktop` | Tauri v2 app shell + TypeScript/React frontend |
| `adagio-cli` | `adagio` CLI binary |
| `adagio-ipc` | Shared IPC types: `DaemonRequest`, `DaemonResponse`, `DaemonClient` |
| `adagio-nextcloud` | Nextcloud WebDAV client + E2EE OCS API client |
| `adagio-e2ee` | E2EE crypto layer: AES-128-GCM, RSA-2048, BIP-39, CMS signatures |
| `adagio-vfs` | FUSE3 virtual filesystem driver |

---

## Feature status

| # | Feature | Status |
|---|---------|--------|
| 001 | Nextcloud file sync | ✅ Done |
| 002 | Desktop app lifecycle | ✅ Done |
| 003 | Account OAuth2 setup | ✅ Done |
| 004 | UI design system | ✅ Done |
| 005 | Conflict resolution wizard | ✅ Done |
| 006 | Sync resource efficiency | ✅ Done |
| 007 | Background sync daemon | ✅ Done |
| 008 | CLI binary | ✅ Done |
| 009 | Bandwidth throttling | ✅ Done |
| 010 | Network awareness | ✅ Done |
| 011 | Bulk upload driver | ✅ Done |
| 012 | VFS on-demand files (FUSE3) | ✅ Done |
| 013 | E2EE encryption | 🔄 In progress |
| 014 | LAN-peer protocol | ⏳ Next |

---

## Requirements

- Rust stable ≥ 1.83
- Node.js ≥ 20 (for the desktop frontend)
- Nextcloud ≥ 28 LTS
- For E2EE: Nextcloud End-to-End Encryption app ≥ 2.0, server-side encryption **disabled** (`occ encryption:disable`)
- For VFS (Linux): `libfuse3-dev`, `fuse3` package

---

## Building

```bash
# Build all crates
cargo build --workspace

# Build and run the daemon
cargo run -p adagio-daemon -- --config-dir ~/.config/ai.neuralagent.adagio

# Build the desktop app (requires Tauri CLI)
cargo tauri dev

# Build the CLI
cargo build -p adagio-cli
```

---

## Development workflow

This project uses [SpecKit](https://github.com/anthropics/speckit) for feature development. Each feature lives in `specs/NNN-feature-name/` with a full spec, implementation plan, data model, and task list.

Architecture decisions are documented in `docs/adr/`. The design system is in `handoff/DESIGN.md`.

---

## Project layout

```
adagio/
├── crates/            # Rust crates (see table above)
├── specs/             # Feature specifications (001–013, active)
├── docs/adr/          # Architecture Decision Records
├── handoff/           # Design system, tokens, screenshots
└── CLAUDE.md          # Current active feature context for AI
```
