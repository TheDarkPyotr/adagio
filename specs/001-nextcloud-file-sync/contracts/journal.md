# Contract: Journal

**Crate**: `adagio-core`
**Module**: `crate::journal`

The `Journal` trait abstracts persistent state tracking for a sync pair. The sync engine
reads and writes journal entries to detect changes across cycles. The SQLite-backed
implementation is the only production implementation; in-memory implementations are
used in tests.

## Trait Definition

```rust
/// Persistent record of the last-known-synced state for all items in a pair.
pub trait Journal: Send + Sync {
    /// Look up the journaled state for a single item by its relative path.
    fn get(
        &self,
        pair_id: &PairId,
        path: &RelativePath,
    ) -> Result<Option<JournalEntry>, JournalError>;

    /// Look up an item by its server-assigned file ID.
    ///
    /// Used to detect renames/moves: if the file ID is known but the path differs,
    /// the item has been renamed.
    fn get_by_file_id(
        &self,
        pair_id: &PairId,
        file_id: &str,
    ) -> Result<Option<JournalEntry>, JournalError>;

    /// Insert or update a journal entry.
    ///
    /// MUST be durable (fsynced) before this call returns. The caller MUST NOT
    /// start any dependent operation until this call succeeds.
    fn upsert(&self, entry: &JournalEntry) -> Result<(), JournalError>;

    /// Remove a journal entry (called after a successful delete operation).
    fn delete(
        &self,
        pair_id: &PairId,
        path: &RelativePath,
    ) -> Result<(), JournalError>;

    /// Return all entries for a pair. Used during full-scan reconciliation and
    /// journal rebuild.
    fn all_entries(
        &self,
        pair_id: &PairId,
    ) -> Result<Vec<JournalEntry>, JournalError>;

    /// Return all entries in the given status for a pair. Used by the errors view
    /// and the activity-log queries.
    fn entries_by_status(
        &self,
        pair_id: &PairId,
        status: &SyncStatus,
    ) -> Result<Vec<JournalEntry>, JournalError>;

    /// Atomically update a batch of entries.
    ///
    /// All updates are applied in a single transaction. If any update fails,
    /// no changes are committed (all-or-nothing).
    fn upsert_batch(&self, entries: &[JournalEntry]) -> Result<(), JournalError>;

    /// Drop all entries for a pair (called when deleting a sync pair).
    fn clear_pair(&self, pair_id: &PairId) -> Result<(), JournalError>;
}
```

## Error Type

```rust
pub enum JournalError {
    /// Unrecoverable: database file corrupt or schema mismatch.
    Corruption(String),
    /// Transient I/O error on the journal file.
    Io(std::io::Error),
    /// Constraint violation (logic bug in caller).
    Constraint(String),
}
```

## Contract Guarantees

1. `upsert` is atomic and durable: it completes an `fsync` (or equivalent) before
   returning. This is the core durability guarantee of the entire sync engine.
2. `upsert_batch` is transactional: either all entries are written or none are.
3. `get` and `all_entries` are read-only and never block writers.
4. `get_by_file_id` returns at most one entry per pair (server file IDs are unique
   per Nextcloud instance).
5. A `JournalError::Corruption` MUST be surfaced as a `SyncError::Fatal` by the engine,
   triggering journal rebuild.
