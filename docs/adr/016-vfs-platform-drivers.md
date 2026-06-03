# ADR-016: VFS Platform Driver Strategy

**Status**: Accepted  
**Date**: 2026-05-30  
**Feature**: 012-vfs-on-demand

---

## Context

VFS (on-demand files) requires OS-level integration to make files appear in the
file manager without downloading content. Three distinct OS APIs exist across
Adagio's supported platforms. The architecture must isolate platform specifics
behind a common interface so the daemon and sync engine remain portable.

---

## Decision

### New crate: `adagio-vfs`

A dedicated `adagio-vfs` crate exposes a `VfsProvider` trait with five operations:
`mount`, `unmount`, `update_placeholders`, `set_locally_available`, `set_pinned`,
`set_cloud_only`. Platform implementations live in cfg-gated modules.

### Platform implementations

| Platform | Mechanism | Crate | Min version |
|----------|-----------|-------|-------------|
| Linux | FUSE3 | `fuse3` (tokio-runtime feature) | Kernel 4.18+, fuse3 package |
| macOS | FileProvider | `objc2-file-provider` | macOS 12 Monterey |
| Windows | Cloud Files API | `wincs` + `windows-rs` | Windows 10 1709 |
| Other | No-op fallback | built-in | — |

### Communication: in-process (not IPC socket)

The FUSE handler runs as a Tokio task inside the daemon and calls the existing
`Arc<dyn RemoteClient>` directly — no extra IPC socket. This reuses all existing
WebDAV download code (including `download_file` with `ByteRange` support) and
avoids the latency and complexity of a second socket.

### Storage: SQLite LRU eviction

Two new tables in adagio.db (migration 003):
- `vfs_cache_metadata` — per-file state, size, last_accessed_at for LRU
- `vfs_pinned_paths` — normalized list of user-pinned paths

LRU eviction uses `ORDER BY last_accessed_at ASC LIMIT N` — no external crate.

---

## Alternatives Considered

| Alternative | Rejected because |
|-------------|-----------------|
| Separate FUSE process with Unix socket | Extra IPC latency; more failure modes; existing daemon restart logic would not cover it |
| In-memory LRU crate | Existing SQLx pool is fast enough; avoids memory-only state that doesn't survive restarts |
| Single flat table for VFS state | `vfs_cache_metadata` is per-file operational state; separating it from `journal_entries` avoids schema bloat and keeps copy-sync queries unchanged |
| Swift extension for macOS | Separate process; IPC overhead; objc2-file-provider provides direct Rust FFI |

---

## Consequences

- `adagio-vfs` crate added to workspace
- `adagio-core` depends on `adagio-vfs` for the `VfsProvider` trait
- Migration 003 adds `vfs_cache_metadata` + `vfs_pinned_paths`; existing tables unchanged
- Three new `DaemonRequest` variants: `GetVfsStats`, `SetVfsPin`, `EvictVfsFile`
- Copy-sync pairs unaffected: `DefaultSyncEngine::start_pair()` checks `pair.vfs_enabled`
