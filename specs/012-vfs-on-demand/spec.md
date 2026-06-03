# Feature Specification: Virtual File System (VFS) — On-demand Files

**Feature ID**: 012  
**Created**: 2026-05-30  
**Status**: Draft  
**Priority**: P1 (browse without download) → P5 (coexistence with copy sync)

---

## Overview

Adagio's copy-sync model requires downloading every file before it can be opened. On a laptop with a 512 GB SSD and a 2 TB Nextcloud library this is impossible — users must either manage complex selective-sync rules or accept that most of their files are inaccessible. VFS integration solves this by making the entire Nextcloud library visible in the file manager immediately, downloading content only when a file is actually opened.

---

## Problem Statement

Users with large Nextcloud libraries (photography archives, video projects, document collections) cannot access all their files on devices with limited storage. Selective sync requires constant manual curation. Copy sync requires more local storage than most devices have. The result is that users either abandon Adagio for services that support on-demand files (iCloud, OneDrive, Dropbox), or limit themselves to a small subset of their content.

---

## User Stories

### US1 — Browse all files without downloading (P1)
As a user with a large Nextcloud library, I want to see all my files immediately after configuring a sync pair, without waiting for downloads or consuming storage for files I haven't accessed.

**Acceptance Criteria:**
- GIVEN a VFS-mode pair and a remote with 10 000 files  
  WHEN the user opens the sync folder  
  THEN all 10 000 entries are visible immediately with correct names, sizes, and modification dates
- Total local storage for metadata is under 10 MB regardless of library size
- Each file entry shows a visual indicator (cloud icon, placeholder badge) that its content is not yet downloaded

### US2 — Transparent on-demand download (P2)
As a user, I want to open any cloud-only file and have it download automatically without any manual steps.

**Acceptance Criteria:**
- GIVEN a file in cloud-only state  
  WHEN a user or application opens it  
  THEN the content downloads transparently; the opening application receives the data as if the file were already local
- After the file is closed, content remains cached locally (locally-available state)
- On download failure (no network), the user sees a clear error message and the file stays in cloud-only state

### US3 — Pin files for offline access (P3)
As a traveller, I want to mark files or folders as "always available" so they download proactively and stay accessible without a network connection.

**Acceptance Criteria:**
- GIVEN any file or folder  
  WHEN the user pins it  
  THEN all content (recursively for folders) downloads immediately in the background
- Pinned files remain accessible with no network connection
- Pinned state persists across app and daemon restarts
- A visual indicator distinguishes pinned files from other locally-available files

### US4 — Evict cached files to free space (P4)
As a user with accumulated cached files, I want to return files to cloud-only state to reclaim local storage without removing them from Nextcloud.

**Acceptance Criteria:**
- GIVEN files in locally-available or pinned state  
  WHEN the user evicts them  
  THEN local content is removed; the file entry remains visible as a cloud placeholder; the file can be re-opened on demand
- Automatic eviction triggers when free disk falls below a configurable threshold, using least-recently-used ordering
- Pinned files are never automatically evicted
- The eviction operation completes and storage is reclaimed within 5 seconds per file

### US5 — Conflict-free coexistence with copy sync (P5)
As a user with existing copy-sync pairs, I want to enable VFS on a new pair without disrupting existing ones.

**Acceptance Criteria:**
- GIVEN a mix of copy-sync and VFS-mode pairs  
  WHEN files change in either pair  
  THEN each pair independently maintains its sync strategy with no interference
- Switching a pair from copy sync to VFS mode is non-destructive: existing local files become locally-available; no data is deleted or re-downloaded

---

## Functional Requirements

### FR-1: File state model
Every file in a VFS pair exists in exactly one of three states:
1. **Cloud-only** — metadata known, zero local content bytes
2. **Locally available** — content cached locally, evictable when storage is low
3. **Pinned** — content cached, never auto-evicted, accessible offline

Valid transitions: cloud-only ↔ locally-available ↔ pinned; pinned → cloud-only only via explicit user eviction.

### FR-2: Metadata population
On first enabling a VFS pair, the full remote directory tree is fetched and stored as placeholder entries. This scan completes before the folder is accessible in the file manager. Subsequent remote changes are detected on the regular sync cycle and reflected in the virtual folder.

### FR-3: On-demand content delivery
When a cloud-only file is opened, the system downloads the content, streams it to the requesting application, caches it locally, and transitions the file to locally-available state. On failure, the application receives an I/O error; the file stays cloud-only.

### FR-4: Pinning and eviction
- **Pin**: applies to individual files or directory trees; pinning a folder covers all current and future files within it
- **Manual eviction**: any locally-available file or tree; pinned files require explicit unpin first
- **Automatic eviction**: triggered when free disk falls below the configured minimum; evicts least-recently-accessed locally-available files until threshold is satisfied; never runs while a file is open

### FR-5: Cache management
- Configurable maximum cache size per pair (default: 20 GB or 50 % of free disk, whichever is smaller)
- The desktop app shows cache usage per pair and allows manual cache clearing
- Cache size counts only downloaded content; metadata is excluded

### FR-6: VFS mode per pair
- Each sync pair independently selects copy sync (default) or VFS mode
- On unsupported platforms or OS versions, VFS mode is hidden and copy sync is the only option
- Mode switches are non-destructive in both directions

### FR-7: Sync cycle integration
VFS pairs participate in the regular sync cycle for remote-change detection. Cycle updates metadata and placeholder entries but does not download content except for pinned files, which are kept current.

### FR-8: Offline behaviour
- Cloud-only files show "not available offline" when opened without network
- Locally-available and pinned files are readable and writable offline
- Writes made offline are queued and synced when connectivity returns, with the same conflict-detection rules as copy sync

---

## Success Criteria

1. **Instant visibility**: A 10 000-file remote is fully browsable within 60 seconds of enabling VFS, with zero content downloaded
2. **Storage efficiency**: Metadata for 10 000 cloud-only files consumes under 10 MB of local storage
3. **Transparent access**: Opening a 10 MB cloud-only file delivers the first byte to the application within 3 seconds on a 10 Mbps connection
4. **Pin reliability**: Pinned files remain accessible after going offline and rebooting the device
5. **Eviction correctness**: Evicting a file reclaims its storage within 5 seconds; the file reappears as a browsable placeholder
6. **Zero disruption**: All existing copy-sync regression tests pass unchanged after VFS is introduced

---

## Key Entities

### VfsFileState (per file, per pair)
- `path`: relative path within the pair
- `state`: cloud-only | locally-available | pinned
- `remote_size`: file size on Nextcloud (bytes)
- `remote_etag`: current remote version identifier
- `remote_mtime`: last modified date on remote
- `cached_at`: timestamp when content was last downloaded (null if cloud-only)
- `last_accessed`: timestamp of last local open (used for LRU eviction ordering)

### VfsPairConfig (per sync pair, persisted)
- `vfs_enabled`: bool
- `cache_max_bytes`: u64 (0 = unlimited)
- `eviction_threshold_bytes`: u64 (free-disk minimum that triggers auto-eviction)
- `pinned_paths`: list of paths kept permanently local

### VfsCacheStats (runtime, not persisted)
- `cloud_only_count`, `locally_available_count`, `pinned_count`
- `cached_bytes`: total bytes of downloaded content
- `last_eviction_at`: timestamp of most recent automatic eviction

---

## Platform Support

| Platform | Mechanism | Minimum OS version |
|----------|-----------|--------------------|
| Linux | FUSE3 | Kernel 4.18+, FUSE3 package installed |
| macOS | FileProvider extension | macOS 12 Monterey |
| Windows | Cloud Files API (CfApi) | Windows 10 version 1709 |

On unsupported platforms or OS versions, VFS mode is unavailable and the UI presents copy sync as the only option.

---

## Assumptions

1. FUSE3 / FileProvider / CfApi is available and functional on the user's device. If not, Adagio falls back to copy sync with a notification.
2. The remote Nextcloud server supports standard WebDAV for metadata and content — no Nextcloud-specific extensions beyond what copy sync already uses.
3. Path names in the Nextcloud library are compatible with the local filesystem. Incompatible names are handled by the same path-compat mechanism as copy sync.
4. Streaming delivery means the OS kernel buffer receives bytes as they arrive from the network. Whether the application can stream depends on the application.
5. The Adagio daemon must be running for on-demand downloads. With the daemon stopped, cloud-only files are inaccessible; locally-available and pinned files remain readable.

---

## Out of Scope (v1)

- Cross-device VFS sync (each device maintains its own cache independently)
- Real-time collaborative editing lock protocol
- Windows production code-signing (test-signed driver acceptable for v1)
- Bandwidth scheduling for proactive pin downloads (Feature 009 bandwidth throttling applies transparently)
- Predictive prefetch based on access patterns
