# Contract: ChangeDetector

**Crate**: `adagio-core`
**Module**: `crate::detection`

`ChangeDetector` abstracts both local filesystem change detection and remote
PROPFIND-based polling. It feeds the Discovery phase of each sync cycle with
current snapshots of both trees.

## Trait Definition

```rust
/// Produces snapshots of the local and remote trees for use in Discovery.
#[async_trait]
pub trait ChangeDetector: Send + Sync {
    /// Build a snapshot of the current local tree, using cached journal data
    /// where mtime + size match to avoid unnecessary checksum computation.
    ///
    /// Only items within the pair's scope (after applying exclude patterns and
    /// selective-sync exclusions) are included.
    async fn local_snapshot(
        &self,
        pair: &SyncPair,
        journal: &dyn Journal,
    ) -> Result<Vec<LocalItem>, DetectorError>;

    /// Build a snapshot of the current remote tree via PROPFIND.
    ///
    /// Implementations SHOULD use the server's efficient change endpoint when
    /// available; otherwise issue a recursive PROPFIND.
    async fn remote_snapshot(
        &self,
        pair: &SyncPair,
        client: &dyn RemoteClient,
    ) -> Result<Vec<RemoteItem>, DetectorError>;

    /// Subscribe to real-time local change events for the pair's local root.
    ///
    /// The returned receiver emits a debounced notification whenever one or
    /// more filesystem events have been coalesced for the given pair.
    /// Receiving a notification triggers a new sync cycle for that pair.
    fn subscribe_local_events(&self, pair: &SyncPair) -> watch::Receiver<LocalChangeSignal>;
}

/// Signals that one or more local changes have occurred and stabilized.
pub struct LocalChangeSignal {
    pub pair_id: PairId,
    pub detected_at: DateTime<Utc>,
}
```

## Contract Guarantees

1. `local_snapshot` MUST NOT modify any local files or journal entries; it is pure
   read.
2. `remote_snapshot` MUST NOT mutate remote state; it is read-only.
3. Both snapshot methods are re-entrant: calling them concurrently for different pairs
   is safe.
4. `subscribe_local_events` is idempotent per pair: calling it twice for the same pair
   returns receivers backed by the same underlying watcher.
5. Events are debounced: a notification is emitted only after the configured quiescence
   window (2–5 s) with no further filesystem events.
