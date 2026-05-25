# Research: Nextcloud File Sync

**Feature**: 001-nextcloud-file-sync
**Date**: 2026-05-24
**Branch**: `001-nextcloud-file-sync`

---

## Decision 1: Desktop GUI Framework

**Decision**: Tauri 2.x

**Rationale**:
- Tauri 2.x ships native system-tray support, native file dialogs, and OS notification
  APIs on all three target platforms (Linux/macOS/Windows) — essential for a sync client.
- The backend (sync engine) is pure Rust; Tauri's IPC bridge exposes it to the webview
  frontend without serialization overhead for large data (streams stay in Rust).
- Tauri bundles are significantly smaller than Electron (5–10 MB vs 60–100 MB) because
  the OS-bundled WebView is used rather than a bundled Chromium.
- Tauri 2.x is stable (released 2024) with an active security model and permission system.

**Alternatives considered**:
- **iced** (pure-Rust immediate-mode GUI): Smaller binary, no webview dependency, but
  lacks system tray / notification support on Windows and has a less mature widget
  ecosystem. Would require custom implementations for file picker dialogs.
- **egui**: Excellent for developer tooling but not polished enough for a production-quality
  sync client UI with system tray integration.
- **GTK4 via gtk4-rs**: Native on Linux, but cross-platform story on macOS/Windows is
  fragile; single-toolkit UI would not feel native on macOS.

**ADR**: `docs/adr/001-gui-framework.md`

---

## Decision 2: Frontend Framework (inside Tauri webview)

**Decision**: Svelte 5 + TypeScript + Vite

**Rationale**:
- Svelte compiles to vanilla JS with minimal runtime; no virtual DOM overhead — best fit
  for the < 100 ms UI response budget.
- TypeScript enforces type safety across the Tauri IPC boundary (Tauri generates typed
  bindings via `@tauri-apps/api`).
- Vite provides fast hot-module replacement during development.
- Svelte's reactivity model maps cleanly to sync state (reactive stores for status/progress).

**Alternatives considered**:
- **React + Vite**: More ecosystem, but larger bundle and more boilerplate for reactive
  state. React 18 concurrent mode adds complexity not needed here.
- **Vue 3**: Comparable to Svelte for this use case but slightly heavier runtime.
- **Vanilla TypeScript**: Fastest, but requires manual DOM management for dynamic lists
  (conflicts view, activity log) — not worth the maintenance cost.

---

## Decision 3: Journal Persistence

**Decision**: SQLite via `sqlx` 0.8 (async, WAL mode, compile-time checked queries)

**Rationale**:
- SQLite WAL mode provides the durability guarantee required: a journal write is durable
  (fsync-visible) before the next dependent operation proceeds, even after a crash.
- `sqlx` provides async access on Tokio without blocking executor threads and supports
  compile-time query verification (`sqlx::query_as!` macros).
- SQLite's single-file format makes the journal easy to inspect, back up, and rebuild.
- A single `journal_entries` table with an index on `(pair_id, relative_path)` handles
  100k entries with sub-millisecond lookups.

**Alternatives considered**:
- **RocksDB / sled**: Better write throughput at scale, but overkill for the target of
  100k entries; no compelling advantage, more operational complexity.
- **Flat JSON / CSV files**: Simple but not crash-safe (no atomic multi-row updates);
  would require a custom WAL, which is reinventing SQLite.
- **diesel**: Compile-time checked queries like `sqlx`, but `diesel` is sync-only and
  would require spawning blocking tasks — less ergonomic with Tokio.

**ADR**: `docs/adr/002-journal-storage.md`

---

## Decision 4: WebDAV Client

**Decision**: `reqwest` 0.12 + manual WebDAV method dispatch + `quick-xml` for PROPFIND

**Rationale**:
- No well-maintained, production-grade Rust WebDAV client library exists (the landscape
  has `dav-server`, `webdav-client`, etc., but none cover Nextcloud's full protocol surface
  including chunked upload assembly).
- `reqwest` is the de-facto async Rust HTTP client; it handles connection pooling, TLS
  (via `rustls` or `native-tls`), redirects, and streaming correctly.
- `quick-xml` is faster than `serde-xml-rs` and handles Nextcloud's PROPFIND namespaces
  correctly in streaming mode (important for large directory listings).
- Wrapping these in `adagio-nextcloud` keeps the Nextcloud-specific protocol surface
  isolated from `adagio-core`.

**Alternatives considered**:
- **`dav-server-fs-inmem`**: Designed for server-side WebDAV serving, not client use.
- **`reqwest-dav`**: Thin wrapper but unmaintained and missing chunked upload support.
- **`hyper` directly**: Lower level than needed; `reqwest` already handles connection
  pooling and redirect logic.

**ADR**: `docs/adr/003-webdav-client.md`

---

## Decision 5: Filesystem Change Detection

**Decision**: `notify` 6.x + `notify-debouncer-full` 0.3

**Rationale**:
- `notify` is the standard cross-platform filesystem event crate in Rust; it wraps
  inotify (Linux), FSEvents (macOS), and ReadDirectoryChangesW (Windows).
- `notify-debouncer-full` implements the 2–5 second quiescence window required by the
  spec, coalescing rapid sequences of events into a single notification per path.
- Complements periodic full scans (at startup and every 2 hours) to catch events dropped
  by OS-level queue limits.

**Alternatives considered**:
- **Manual inotify/FSEvents/WinAPI bindings**: Platform-specific, high maintenance burden.
- **Polling-only**: Simpler but fails the 5-second detection latency requirement during
  active use.

---

## Decision 6: Credential Storage

**Decision**: `keyring` 3.x crate

**Rationale**:
- `keyring` provides a unified API over macOS Keychain, Windows Credential Manager
  (DPAPI), and Linux Secret Service (via D-Bus or file-backed fallback).
- Satisfies FR-002: credentials never in plain-text config files or logs.
- OAuth2 refresh tokens and app-passwords are stored under a service name
  (`adagio/<account-id>`) and retrieved at runtime.

---

## Decision 7: Checksum Algorithms

**Decision**: SHA-256 (primary) via `sha2` crate; MD5 fallback via `md5` crate

**Rationale**:
- Nextcloud servers announce their preferred checksum algorithm via the capabilities
  endpoint (`checksums.supportedTypes`). SHA-256 is preferred; MD5 is the legacy
  fallback for older Nextcloud instances.
- `sha2` from the RustCrypto project is well-audited, zero-allocation in streaming mode
  via the `Digest` trait.
- Checksums are computed during streaming I/O (incremental `Digest::update`) with no
  requirement to load the full file.

---

## Decision 8: Structured Logging

**Decision**: `tracing` + `tracing-subscriber` with JSON formatter for production,
  pretty-print for development

**Rationale**:
- `tracing` is the standard async-aware instrumentation crate in the Tokio ecosystem;
  spans automatically propagate through `async`/`await` chains, giving per-operation
  context to all child log records.
- JSON output (via `tracing-subscriber::fmt::json()`) satisfies Constitution Principle
  III: structured logs at configurable verbosity levels with no credentials or file
  contents.
- `RUST_LOG` env var controls verbosity at runtime; log files are rotated via
  `tracing-appender`.

---

## Decision 9: Nextcloud Chunked Upload Protocol

**Decision**: Nextcloud's proprietary chunked upload (upload to `.uploads/` temp folder
via sequential PUTs, then MOVE to final path)

**Rationale**:
- The Nextcloud server's capabilities endpoint announces `chunkedupload` support.
- The protocol uploads chunks to `<remote_root>/.uploads/<upload-id>/<chunk-index>`,
  then issues a `MOVE` to assemble them at the final path.
- This is distinct from the TUS protocol (which Nextcloud also supports on newer
  instances); the proprietary chunked protocol is more universally available.
- Resume is possible because chunk indices are stable: the client checks which chunks
  exist in the upload session before sending.

---

## Open Questions Resolved

| Question | Resolution |
|----------|-----------|
| GUI framework | Tauri 2.x (see Decision 1) |
| Frontend framework | Svelte 5 + TypeScript (see Decision 2) |
| Journal storage | SQLite via sqlx (see Decision 3) |
| WebDAV client | reqwest + quick-xml (see Decision 4) |
| Filesystem events | notify + debouncer (see Decision 5) |
| Credential storage | keyring 3.x (see Decision 6) |
| Checksum algorithm | SHA-256 primary, MD5 fallback (see Decision 7) |
| Structured logging | tracing + tracing-subscriber (see Decision 8) |
| Chunked upload protocol | NC proprietary chunked upload (see Decision 9) |
| TLS | `rustls` via reqwest (no native-tls; avoid OpenSSL dep) |
| Config file format | TOML via `toml` crate (human-editable, well-supported) |
| Error handling | `thiserror` for library errors; `anyhow` for application code |
| Async channels | `tokio::sync::mpsc` for event pipelines; `tokio::sync::watch` for status broadcast |
| Virtual files mode | Out of scope for this phase; deferred to separate feature |
