# Contract: SyncEngine

**Crate**: `adagio-core`
**Module**: `crate::cycle`

The `SyncEngine` trait is the primary interface consumers use to control sync lifecycle
for a single `SyncPair`. The `adagio-desktop` Tauri commands call this trait exclusively;
they never reach into the sync cycle internals directly.

## Trait Definition

```rust
/// Controls the lifecycle of sync for a single sync pair.
#[async_trait]
pub trait SyncEngine: Send + Sync {
    /// Start a sync cycle for the given pair.
    ///
    /// If a cycle is already running for this pair, the request is coalesced:
    /// no new cycle is started, but the caller receives the existing cycle's
    /// completion notification.
    async fn trigger_sync(&self, pair_id: PairId) -> Result<SyncReport, SyncError>;

    /// Pause all sync activity for the given pair.
    /// In-flight transfers are allowed to complete their current chunk.
    async fn pause(&self, pair_id: PairId) -> Result<(), SyncError>;

    /// Resume a paused pair. No-op if the pair is not paused.
    async fn resume(&self, pair_id: PairId) -> Result<(), SyncError>;

    /// Return the current status of the pair without blocking.
    fn status(&self, pair_id: PairId) -> PairStatus;

    /// Subscribe to status updates for a pair.
    /// The returned receiver emits a new value whenever the pair's status changes.
    fn subscribe_status(&self, pair_id: PairId) -> watch::Receiver<PairStatus>;

    /// Return the N most-recent activity log entries for a pair.
    async fn activity_log(
        &self,
        pair_id: PairId,
        limit: usize,
    ) -> Result<Vec<ActivityEntry>, SyncError>;

    /// Return all items currently in error state for a pair.
    async fn error_items(
        &self,
        pair_id: PairId,
    ) -> Result<Vec<ErrorItem>, SyncError>;
}
```

## Associated Types

```rust
pub struct SyncReport {
    pub pair_id: PairId,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub uploaded: u64,    // item count
    pub downloaded: u64,
    pub deleted: u64,
    pub conflicts: u64,
    pub errors: u64,
}

pub struct ActivityEntry {
    pub timestamp: DateTime<Utc>,
    pub relative_path: RelativePath,
    pub operation: OperationType,
    pub direction: Option<TransferDirection>,
    pub result: OperationResult,
}

pub struct ErrorItem {
    pub relative_path: RelativePath,
    pub operation: OperationType,
    pub category: ErrorCategory,
    pub message: String,
    pub retry_count: u8,
}
```

## Contract Guarantees

1. `trigger_sync` is idempotent for concurrent callers — at most one cycle runs per pair
   at a time.
2. `pause` does not lose data: in-flight chunk uploads complete their current chunk
   before the pause takes effect.
3. `status` is lock-free (backed by a `watch::Receiver` snapshot) and always returns
   immediately.
4. All async methods are cancellation-safe: dropping the future does not corrupt the
   journal or leave partial local files.
