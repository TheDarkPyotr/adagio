# Research: VFS On-demand Files (012)

## 1. Linux FUSE3

### Decision: `fuse3` crate (v0.9+) with Tokio runtime feature

**Rationale**: Pure Rust, no C `libfuse` dependency (except for unprivileged mounting via `fusermount3`), native async-Tokio integration. Supports `readdirplus` for efficient directory listing with attributes in a single round-trip. fuse3 auto-manages kernel ↔ userspace threads.

**Minimum required FUSE ops:**
- `lookup(parent, name)` → resolves a name to an inode
- `getattr(inode)` → returns file metadata (size, mtime, permissions)
- `readdir(inode)` / `readdirplus(inode)` → directory listing with optional attrs
- `open(inode, flags)` → called before read; can block for metadata hydration
- `read(inode, offset, size)` → called per-chunk (~64–128 KB); blocks the caller until data arrives; handler awaits download then returns bytes

**On-demand delivery model**: FUSE `read()` blocks the requesting userspace process until the handler returns. The FUSE handler awaits a download future (`tokio::spawn_blocking` or direct async) and returns the fetched bytes. Callers see no difference from a local read.

**State representation**: Extended attributes (`user.adagio.state`) on placeholder file entries for per-file state visible to other tools; authoritative state in the SQLite `vfs_cache_metadata` table.

**Alternatives considered**: `fuser` (sync-only), `polyfuse` (abandoned).

---

## 2. macOS FileProvider (macOS 12+)

### Decision: `objc2-file-provider` Rust bindings (direct FFI, no Swift/ObjC extension process)

**Rationale**: `objc2-file-provider` provides auto-generated safe Rust bindings to Apple's FileProvider framework. No separate Swift process required; the Tauri app embeds the provider logic via Rust FFI. macOS 12 Monterey is the minimum for the replicated extension API.

**Key FileProvider APIs:**
- `NSFileProviderReplicatedExtension.providePlaceholder(at:)` → creates a ~1 KB placeholder file entry with metadata
- `NSFileProviderReplicatedExtension.fetchContents(for:version:request:completionHandler:)` → hydration callback invoked when the kernel intercepts a read on a placeholder
- `NSFileProviderEnumerator` → provides directory listings to Finder/Spotlight
- The provider is registered as a domain; the main app calls `NSFileProviderManager.add(_:completionHandler:)` on launch

**Entitlements required**: `com.apple.developer.fileprovider.unrestricted-access` (sandbox exemption for provider), `NSExtensionPointIdentifier: com.apple.fileprovider-nonui`, app groups sharing.

**Alternatives considered**: Swift extension process (separate process, IPC overhead), Rclone FUSE on macOS (no native Finder integration, no pinning/eviction UI).

---

## 3. Windows Cloud Files API (CfApi, Windows 10 1709+)

### Decision: `wincs` crate (safe CfApi abstractions) + `windows-rs` for types

**Rationale**: `wincs` provides idiomatic Rust abstractions over CfApi — sync root registration, placeholder creation, and the hydration callback loop. No kernel driver signing required for development; `cldflt.sys` (the Windows cloud filter minifilter) ships in Windows and handles kernel interception.

**Key CfApi concepts:**
- `CfRegisterSyncRoot()` → registers the folder as a cloud sync root (appears in File Explorer with cloud badge)
- `CfCreatePlaceholders()` → bulk-creates placeholder entries from file metadata
- `CF_CALLBACK_TYPE_FETCH_DATA` → hydration callback fired when an app reads a placeholder; the provider calls `CfExecute(CF_OPERATION_TYPE_TRANSFER_DATA)` to stream bytes
- `CfSetPinState()` → sets/clears pinned (offline) state visible in File Explorer context menu
- 60-second timeout per hydration request; provider must respond or fail gracefully

**No driver signing needed**: CfApi uses the in-box `cldflt.sys` minifilter. Desktop Bridge packaging (`Package.appxmanifest` with `CloudFiles` capability) is required for Store distribution but not development.

**Alternatives considered**: Dokan (third-party FUSE-like layer, extra install), WinFsp (FUSE on Windows, no CfApi shell integration).

---

## 4. VFS Architecture

### Decision: Separate `adagio-vfs` crate; VFS pair runner replaces propagator for VFS-mode pairs; FUSE↔daemon via existing IPC socket

**VFS pair runner differentiation**: Same metadata-sync cycle as copy-sync (reconciler detects remote changes, updates journal/placeholders) but the propagator step is replaced by `VfsMetadataSync` — updating placeholder entries, evicting stale files, and queuing pinned-path downloads. Content downloads happen only on-demand via FUSE callbacks.

**FUSE↔daemon communication**: The FUSE handler process connects to the existing Unix domain socket (`adagio-ipc`) to send `FetchOnDemand { pair_id, path, byte_range }` requests. The daemon dispatcher routes these to the existing transfer engine and streams results back. This reuses all existing Nextcloud WebDAV download logic.

**LRU eviction**: SQLite `ORDER BY last_accessed_at ASC LIMIT N` query on `vfs_cache_metadata`. No external crate needed — the existing SQLx pool handles it efficiently with a composite index on `(pair_id, last_accessed_at)`.

**State storage**: New migration adds `vfs_cache_metadata` table (path, size_bytes, last_accessed_at, pinned) and `vfs_pinned_paths` table (pair_id, path, pinned_at) to the existing adagio.db.

---

## 5. ADR to record

**ADR-016**: Three-platform VFS implementation — `fuse3` (Linux), `objc2-file-provider` (macOS), `wincs` (Windows). FUSE handler is an in-daemon async task communicating with the existing transfer engine via `Arc<dyn RemoteClient>`. New `adagio-vfs` crate isolates platform-specific code behind a common `VfsProvider` trait.
