# Implementation Plan: Nextcloud File Sync

**Branch**: `001-nextcloud-file-sync` | **Date**: 2026-05-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/001-nextcloud-file-sync/spec.md`

## Summary

Adagio implements a cross-platform, bidirectional Nextcloud file sync engine in Rust.
The engine detects local changes (via filesystem events + periodic scans) and remote
changes (via WebDAV PROPFIND polling), reconciles them into a deterministic operation
plan, and executes uploads, downloads, moves, and deletions with full resumability,
checksum verification, and conflict resolution. A Cargo workspace separates the headless
sync engine crate from the Nextcloud protocol crate and the Tauri desktop shell,
ensuring the core can be tested independently of any UI.

See [research.md](research.md) for all technology decisions and ADR references.
See [data-model.md](data-model.md) for entity definitions.
See [contracts/](contracts/) for public trait boundaries.
See [quickstart.md](quickstart.md) for developer setup.

## Technical Context

**Language/Version**: Rust stable, edition 2021, MSRV 1.78+

**Primary Dependencies**:
- `tokio` 1.x — async runtime (executor for all async ops)
- `reqwest` 0.12 — HTTP/HTTPS client for WebDAV calls
- `quick-xml` 0.36 — PROPFIND XML parsing
- `notify` 6.x + `notify-debouncer-full` — cross-platform filesystem events
- `sqlx` 0.8 (SQLite feature) — async journal persistence
- `keyring` 3.x — OS-native credential store (macOS Keychain, Windows DPAPI, Linux SecretService)
- `tauri` 2.x — cross-platform desktop shell
- `svelte` + `vite` — frontend UI (inside Tauri webview)
- `serde` + `serde_json` — serialization for IPC and config
- `sha2` 0.10 — SHA-256 checksums (primary)
- `md5` — MD5 checksums (Nextcloud legacy compatibility)
- `tracing` + `tracing-subscriber` — structured logging
- `tokio-util` — streaming I/O codecs for chunked transfers
- `bytes` — zero-copy byte buffer plumbing
- `criterion` — micro-benchmarks for hot paths

**Storage**: SQLite (via `sqlx`, WAL mode) for journals; OS keychain for credentials;
JSON files for sync pair configuration under `$XDG_CONFIG_HOME/adagio/` (Linux),
`~/Library/Application Support/adagio/` (macOS), `%APPDATA%\adagio\` (Windows)

**Testing**:
- `cargo test` — unit tests, contract tests (in-process)
- `cargo test --test integration` — integration tests requiring a live Nextcloud instance
- `cargo criterion` — benchmark suite for hot paths

**Target Platform**: Linux x86_64/aarch64, macOS arm64/x86_64, Windows x86_64

**Project Type**: desktop-app (Tauri 2.x shell) + internal crate library (sync engine)

**Performance Goals**:
- Idle RSS < 100 MB (clean sync state)
- Background steady-state CPU ≤ 5% on a modern laptop core
- UI action feedback ≤ 100 ms (all user-triggered actions)
- Local change detection latency ≤ 5 s after quiescence window
- Remote change polling interval default: 30 s (active), 5 min (idle)

**Constraints**:
- Uploads and downloads MUST stream from/to disk — no full-file in-memory loading
- Journal writes for completed ops MUST be durable before dependent ops start
- Journal MUST reside outside the sync pair's local root
- Safe to kill at any time — next startup reconciles and resumes without data loss
- Cross-platform path incompatibilities MUST be detected before propagation, never silently fixed

**Scale/Scope**:
- Multiple accounts per device; multiple sync pairs per account
- Target: up to 100,000 sync items and 50 GB per sync pair
- Bounded concurrency: 3 up + 3 down (configurable), total ≤ 6 active transfers

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Verify each gate; mark ✅ pass / ❌ fail / N/A:

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ Enforced by tasks.md: test tasks precede impl tasks |
| All public Rust items have `///` doc comments | II. Documentation as Code | ✅ CI `cargo doc --no-deps` check planned |
| ADR recorded in `docs/adr/` for significant design decisions | II. Documentation as Code | ✅ ADRs 001–003 planned in research.md |
| Structured logging added to all new sync/network operations | III. Observability | ✅ `tracing` instrumentation required in all crates |
| No `println!` in production code paths | III. Observability | ✅ `cargo clippy` will flag; `tracing` macros used instead |
| New feature implemented as independent crate/module with no direct coupling to core | IV. Extensibility | ✅ `adagio-core` / `adagio-nextcloud` / `adagio-desktop` crates are independent |
| Cross-module calls go through defined trait/interface contracts | IV. Extensibility | ✅ `RemoteClient`, `Journal`, `SyncEngine` traits in contracts/ |
| Idle memory budget <100 MB RSS confirmed or N/A for this feature | V. Performance-Oriented | ✅ Headless Rust engine; benchmark gate in CI |
| UI actions provide feedback within 100 ms confirmed or N/A | V. Performance-Oriented | ✅ Tauri commands are async; spinner emitted before any I/O |
| Benchmarks added for any hot-path changes (sync diff, file I/O, network) | V. Performance-Oriented | ✅ `criterion` benchmarks planned for reconciler and checksum |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ CI gate on all three platforms |
| `cargo fmt --check` passes | Dev Workflow | ✅ CI gate |
| All `unsafe` blocks have `// SAFETY:` comments | Dev Workflow | ✅ Enforced in code review checklist |
| All three platform CI targets (Linux, macOS, Windows) pass | Technology | ✅ GitHub Actions matrix planned |

**Constitution Check result: All gates pass. Proceed to Phase 0.**

*Post-Phase-1 re-check: all gates remain valid. Crate boundaries confirmed in Project
Structure below; trait contracts defined in contracts/.*

## Project Structure

### Documentation (this feature)

```text
specs/001-nextcloud-file-sync/
├── plan.md              # This file
├── research.md          # Phase 0: technology decisions and ADR references
├── data-model.md        # Phase 1: entity definitions and state machines
├── quickstart.md        # Phase 1: developer setup guide
├── contracts/           # Phase 1: public trait boundaries
│   ├── sync-engine.md
│   ├── remote-client.md
│   ├── journal.md
│   ├── change-detector.md
│   └── transfer-manager.md
├── checklists/
│   └── requirements.md  # Spec quality checklist (already complete ✅)
└── tasks.md             # Phase 2: generated by /speckit-tasks (not yet created)
```

### Source Code (repository root)

```text
adagio/                             # Cargo workspace root
├── Cargo.toml                      # [workspace] members + resolver = "2"
├── Cargo.lock
├── crates/
│   ├── adagio-core/                # Sync engine — no UI, no platform-specific code
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs           # SyncPair, ExcludePattern, ConflictPolicy config
│   │       ├── journal/
│   │       │   ├── mod.rs          # Journal trait definition
│   │       │   └── sqlite.rs       # SQLite-backed JournalStore impl
│   │       ├── detection/
│   │       │   ├── mod.rs          # ChangeDetector trait
│   │       │   ├── local.rs        # notify events + periodic scan
│   │       │   └── remote.rs       # WebDAV PROPFIND polling
│   │       ├── cycle/
│   │       │   ├── mod.rs          # SyncEngine trait, SyncCycle orchestrator
│   │       │   ├── discovery.rs    # builds local + remote snapshots
│   │       │   ├── reconciler.rs   # produces OperationPlan (pure, no I/O)
│   │       │   └── propagator.rs   # executes plan with bounded concurrency
│   │       ├── transfer/
│   │       │   ├── mod.rs          # TransferManager trait
│   │       │   ├── upload.rs       # single PUT + chunked upload
│   │       │   └── download.rs     # streaming download with range-request resume
│   │       ├── conflict.rs         # conflict detection and resolution policies
│   │       ├── bandwidth.rs        # rate limiter + time-window scheduler
│   │       ├── path_compat.rs      # cross-platform path validation
│   │       ├── error.rs            # SyncError enum (transient/permanent/fatal)
│   │       └── observability.rs    # ActivityLog, SyncStatus, DiagnosticBundle
│   │
│   ├── adagio-nextcloud/           # Nextcloud protocol extensions
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── auth.rs             # app-password + OAuth2 PKCE flow
│   │       ├── capabilities.rs     # /ocs/v2.php/cloud/capabilities parsing
│   │       ├── webdav.rs           # PROPFIND, MOVE, MKCOL, DELETE, COPY
│   │       └── chunked.rs          # NC chunked upload (upload sessions + MOVE)
│   │
│   └── adagio-desktop/             # Tauri 2.x desktop application
│       ├── Cargo.toml
│       ├── tauri.conf.json
│       ├── src/                    # Rust backend
│       │   ├── main.rs
│       │   ├── state.rs            # AppState (Arc<SyncManager>)
│       │   └── commands/           # Tauri IPC commands
│       │       ├── account.rs      # add_account, remove_account, re_auth
│       │       ├── pair.rs         # create_pair, update_pair, delete_pair
│       │       ├── sync.rs         # get_status, pause_sync, resume_sync
│       │       └── conflicts.rs    # list_conflicts, resolve_conflict
│       └── src-ui/                 # Svelte + TypeScript frontend
│           ├── App.svelte
│           ├── lib/
│           │   ├── api.ts          # typed wrappers around Tauri IPC
│           │   └── stores.ts       # Svelte stores for sync state
│           └── routes/
│               ├── Dashboard.svelte
│               ├── Pairs.svelte
│               ├── Conflicts.svelte
│               └── Settings.svelte
│
├── benches/
│   ├── reconciler.rs               # criterion benchmark: reconciler hot path
│   └── checksum.rs                 # criterion benchmark: SHA-256 throughput
│
├── tests/
│   ├── integration/
│   │   ├── sync_cycle.rs           # full sync cycle against live Nextcloud
│   │   ├── transfer.rs             # upload/download/resume tests
│   │   ├── conflict.rs             # conflict detection and resolution
│   │   └── error_recovery.rs       # transient error retry + crash recovery
│   └── contract/
│       ├── journal_contract.rs     # Journal trait contract tests
│       └── remote_client_contract.rs # RemoteClient trait contract tests
│
└── docs/
    ├── adr/
    │   ├── 001-gui-framework.md    # Decision: Tauri 2.x
    │   ├── 002-journal-storage.md  # Decision: SQLite via sqlx
    │   └── 003-webdav-client.md    # Decision: reqwest + quick-xml
    └── quickstart.md               # → symlink or copy of specs/.../quickstart.md
```

**Structure Decision**: Cargo workspace with three member crates (`adagio-core`,
`adagio-nextcloud`, `adagio-desktop`). This satisfies Constitution Principle IV
(Extensibility) by keeping the sync engine completely decoupled from the UI and from
Nextcloud-specific protocol concerns. Future integrations (Notes, Talk, Calendar) each
become a new `adagio-<service>` crate without touching core.

## Complexity Tracking

> No Constitution Check violations. Section not required.
