# Implementation Plan: VFS On-demand Files

**Branch**: `012-vfs-on-demand` | **Date**: 2026-05-30 | **Spec**: [spec.md](spec.md)

---

## Summary

Add a new `adagio-vfs` crate that provides a platform-abstract `VfsProvider` trait and three platform implementations (FUSE3 / FileProvider / CfApi). VFS-mode sync pairs populate placeholder file entries from Nextcloud metadata and deliver content only when files are actually opened. The FUSE handler runs as an async Tokio task inside the daemon, calling the existing transfer engine directly for on-demand downloads.

**Key ADR**: ADR-016 — `fuse3` (Linux), `objc2-file-provider` (macOS 12+), `wincs` (Windows 10 1709+). In-process communication (Arc<dyn RemoteClient>) not separate IPC socket. SQLite LRU eviction via `ORDER BY last_accessed_at`. New `vfs_cache_metadata` and `vfs_pinned_paths` tables in existing adagio.db.

---

## Technical Context

**Language**: Rust stable + Tokio  
**New crate**: `crates/adagio-vfs` — platform-specific VFS drivers behind `VfsProvider` trait  
**New dependencies**:
- `fuse3 = "0.9"` (with `tokio-runtime` feature) — Linux
- `objc2-file-provider` — macOS 12+
- `wincs = "0.2"` + `windows-rs` — Windows 10 1709+

**Storage**: SQLite migration 003 — `vfs_cache_metadata` + `vfs_pinned_paths` tables  
**Testing**: Unit tests with `MockVfsProvider`; integration tests on Linux only (FUSE3 in CI)  
**Platform CI**: Linux path fully testable; macOS and Windows paths require platform runners  
**Performance Goals**: 10 000-file directory browseable < 60 s; first byte of on-demand read < 3 s on 10 Mbps

---

## Constitution Check

| Gate | Principle | Status |
|------|-----------|--------|
| Tests before implementation | I. Test-First | ✅ planned |
| Public Rust items documented | II. Documentation as Code | ✅ enforced |
| ADR-016 in `docs/adr/` | II. Documentation as Code | ✅ planned |
| Structured logging for VFS ops | III. Observability | ✅ planned |
| `adagio-vfs` isolated crate, no direct coupling to daemon | IV. Extensibility | ✅ planned |
| `VfsProvider` trait boundary | IV. Extensibility | ✅ planned |
| VFS metadata fetch < 60 s for 10 k files | V. Performance-Oriented | ✅ designed |
| Idle VFS daemon memory within 100 MB RSS | V. Performance-Oriented | ✅ FUSE in-process |
| `cargo clippy -- -D warnings` | Dev Workflow | ✅ enforced |
| All three platform CI targets | Technology | ⚠️ macOS/Win need platform runners |

---

## Architecture

```
crates/adagio-vfs/
├── src/
│   ├── lib.rs           — VfsProvider trait, VfsError, VfsState, VfsCacheEntry
│   ├── linux.rs         — fuse3 implementation (cfg(target_os = "linux"))
│   ├── macos.rs         — FileProvider via objc2 (cfg(target_os = "macos"))
│   ├── windows.rs       — CfApi via wincs (cfg(windows))
│   └── fallback.rs      — no-op implementation for unsupported platforms
│
crates/adagio-core/src/
├── vfs/
│   ├── mod.rs           — VfsPairRunner, VfsMetadataSync, cache management
│   ├── journal.rs       — VfsJournal trait extension (vfs_cache_metadata queries)
│   └── eviction.rs      — LRU eviction logic
│
crates/adagio-core/migrations/
└── 003_vfs_tables.sql   — vfs_cache_metadata + vfs_pinned_paths

crates/adagio-daemon/src/
└── dispatcher.rs        — 3 new handlers: GetVfsStats, SetVfsPin, EvictVfsFile

crates/adagio-ipc/src/types.rs
└── + 3 new DaemonRequest variants

crates/adagio-desktop/src/
├── config/mod.rs        — vfs_enabled, vfs_cache_max_bytes, vfs_eviction_threshold_bytes
└── commands/vfs.rs      — 3 Tauri command handlers

crates/adagio-desktop/src-ui/src/
├── tauri.ts             — VfsStatsDto + 3 bindings
└── components/PairsScene.tsx  — VFS toggle + cache usage in pair card

docs/adr/016-vfs-platform-drivers.md

crates/adagio-cli/src/handlers/vfs.rs   — adagio vfs status|pin|unpin|evict
```

---

## File Structure

| File | Action |
|------|--------|
| `crates/adagio-vfs/` | CREATE — new crate |
| `crates/adagio-core/src/vfs/` | CREATE — VfsPairRunner, cache management |
| `crates/adagio-core/migrations/003_vfs_tables.sql` | CREATE |
| `crates/adagio-core/src/cycle/mod.rs` | MODIFY — route VFS pairs to VfsPairRunner |
| `crates/adagio-core/src/types.rs` | MODIFY — 3 vfs fields on SyncPair |
| `crates/adagio-desktop/src/config/mod.rs` | MODIFY — 3 vfs fields on SavedPair |
| `crates/adagio-daemon/src/dispatcher.rs` | MODIFY — 3 new handlers |
| `crates/adagio-ipc/src/types.rs` | MODIFY — 3 new DaemonRequest variants |
| `crates/adagio-desktop/src/commands/vfs.rs` | CREATE — 3 Tauri commands |
| `crates/adagio-desktop/src/lib.rs` | MODIFY — register vfs commands |
| `crates/adagio-desktop/src-ui/src/tauri.ts` | MODIFY — VfsStatsDto + 3 bindings |
| `crates/adagio-desktop/src-ui/src/components/PairsScene.tsx` | MODIFY — VFS toggle + stats |
| `crates/adagio-cli/src/handlers/vfs.rs` | CREATE — vfs subcommand |
| `docs/adr/016-vfs-platform-drivers.md` | CREATE |

---

## Key Design Decisions

### VfsProvider trait (adagio-vfs/src/lib.rs)
```rust
#[async_trait]
pub trait VfsProvider: Send + Sync + 'static {
    fn is_supported() -> bool where Self: Sized;
    async fn mount(&self, mount_point: &Path, pair: &SyncPair,
                   client: Arc<dyn RemoteClient>, journal: Arc<dyn Journal>)
                   -> Result<VfsMountHandle, VfsError>;
}

pub struct VfsMountHandle {
    unmount_tx: tokio::sync::oneshot::Sender<()>,
}
```

### VfsPairRunner vs copy PairRunner
```
copy PairRunner:  scan → reconcile → propagate (download/upload content)
VFS PairRunner:   scan → reconcile → update_placeholders (metadata only)
                  + background: process pin-download queue
                  + background: run LRU eviction if disk low
```

### On-demand download (FUSE read() handler)
```rust
// Inside FUSE handler — awaited synchronously (blocks the kernel request):
async fn on_read(&self, path: RelativePath, offset: u64, size: u64) -> Bytes {
    // 1. Check cache: is content already on disk?
    if let Some(cached) = self.cache.read(path, offset, size).await { return cached; }
    // 2. Download range via existing transfer engine
    let bytes = self.client.download(&remote_path, Some(ByteRange { start: offset, end: offset + size })).await?;
    // 3. Write to cache + update vfs_cache_metadata.last_accessed_at
    self.cache.write(path, offset, &bytes).await?;
    bytes
}
```

### Config additions (with #[serde(default)])
```rust
pub vfs_enabled: bool,                      // default false
pub vfs_cache_max_bytes: u64,               // default 20 GiB
pub vfs_eviction_threshold_bytes: u64,      // default 5 GiB
```

---

## Shell Commands

```bash
cargo build -p adagio-vfs -p adagio-core -p adagio-daemon -p adagio-cli
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace -- -D warnings
```
