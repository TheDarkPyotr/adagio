# IPC Contract: VFS On-demand Files (012)

All new variants follow the existing NDJSON IPC protocol (ADR-009).

---

## New DaemonRequest variants

### GetVfsStats
```json
{ "method": "get_vfs_stats", "params": { "pair_id": "..." } }
```
**Response** — `DaemonResponse::Status(Value)`:
```json
{
  "pair_id": "ecfaf3c2-...",
  "cloud_only_count": 9850,
  "locally_available_count": 120,
  "pinned_count": 30,
  "cached_bytes": 536870912,
  "cache_max_bytes": 21474836480,
  "last_eviction_at": "2026-05-30T14:00:00Z"
}
```

---

### SetVfsPin
```json
{
  "method": "set_vfs_pin",
  "params": { "pair_id": "...", "path": "Documents/report.pdf", "pinned": true }
}
```
- `pinned: true` → pin the path (file or directory)
- `pinned: false` → unpin (file returns to locally-available if cached, cloud-only if not)

**Response** — `DaemonResponse::Unit {}` on success.

---

### EvictVfsFile
```json
{
  "method": "evict_vfs_file",
  "params": { "pair_id": "...", "path": "Photos/vacation/IMG_001.jpg" }
}
```
- Fails if the path is pinned (returns error: "path is pinned, unpin first")
- Removes local content; sets state to cloud_only

**Response** — `DaemonResponse::Unit {}` on success.

---

## New Tauri commands

| Command | Params | Return |
|---------|--------|--------|
| `get_vfs_stats` | `pairId: string` | `VfsStatsDto` |
| `set_vfs_pin` | `pairId, path, pinned: bool` | `void` |
| `evict_vfs_file` | `pairId, path` | `void` |

---

## FUSE↔daemon internal protocol (in-process via Arc, not socket)

Based on ADR-016 decision: the FUSE handler runs as a Tokio task inside the daemon process and calls the transfer engine directly via `Arc<dyn RemoteClient>`. No separate IPC socket for on-demand downloads.

```rust
// Called by FUSE read() handler, awaited inline:
async fn fetch_content(
    client: &Arc<dyn RemoteClient>,
    journal: &Arc<dyn Journal>,
    pair_id: &PairId,
    path: &RelativePath,
    offset: u64,
    length: u64,
) -> Result<Bytes, VfsError>
```

This reuses the existing `download_file` transfer function with byte-range support (`ByteRange { start, end }`).

---

## TypeScript types (tauri.ts additions)

```typescript
export interface VfsStatsDto {
  pair_id: string;
  cloud_only_count: number;
  locally_available_count: number;
  pinned_count: number;
  cached_bytes: number;
  cache_max_bytes: number;
  last_eviction_at: string | null;
}

export const getVfsStats = (pairId: string): Promise<VfsStatsDto> =>
  invoke('get_vfs_stats', { pairId });

export const setVfsPin = (pairId: string, path: string, pinned: boolean): Promise<void> =>
  invoke('set_vfs_pin', { pairId, path, pinned });

export const evictVfsFile = (pairId: string, path: string): Promise<void> =>
  invoke('evict_vfs_file', { pairId, path });
```

---

## CLI additions

```
adagio vfs status [--pair-id ID]    Show cache stats for VFS pairs
adagio vfs pin <path>               Pin a path for offline access
adagio vfs unpin <path>             Unpin a path
adagio vfs evict <path>             Evict a path's local content
adagio vfs evict --all              Evict all locally-available content for a pair
```
