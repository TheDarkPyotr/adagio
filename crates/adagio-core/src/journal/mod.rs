use crate::error::JournalError;
use crate::types::{
    ConflictRecord, ConflictResolution, JournalEntry, PairId, RelativePath, SyncStatus,
};
use async_trait::async_trait;

pub mod corruption;
pub mod sqlite;

/// Persistent record of the last-known-synced state for all items in a pair.
///
/// The SQLite-backed implementation is the only production implementation;
/// in-memory implementations are used in tests.
#[async_trait]
pub trait Journal: Send + Sync {
    /// Look up the journaled state for a single item by its relative path.
    async fn get(
        &self,
        pair_id: &PairId,
        path: &RelativePath,
    ) -> Result<Option<JournalEntry>, JournalError>;

    /// Look up an item by its server-assigned file ID.
    ///
    /// Used to detect renames/moves: if the file ID is known but the path differs,
    /// the item has been renamed.
    async fn get_by_file_id(
        &self,
        pair_id: &PairId,
        file_id: &str,
    ) -> Result<Option<JournalEntry>, JournalError>;

    /// Insert or update a journal entry.
    ///
    /// MUST be durable (fsynced) before this call returns. The caller MUST NOT
    /// start any dependent operation until this call succeeds.
    async fn upsert(&self, entry: &JournalEntry) -> Result<(), JournalError>;

    /// Remove a journal entry (called after a successful delete operation).
    async fn delete(&self, pair_id: &PairId, path: &RelativePath) -> Result<(), JournalError>;

    /// Return all entries for a pair. Used during full-scan reconciliation and
    /// journal rebuild.
    async fn all_entries(&self, pair_id: &PairId) -> Result<Vec<JournalEntry>, JournalError>;

    /// Return all entries in the given status for a pair. Used by the errors view
    /// and the activity-log queries.
    async fn entries_by_status(
        &self,
        pair_id: &PairId,
        status: &SyncStatus,
    ) -> Result<Vec<JournalEntry>, JournalError>;

    /// Atomically update a batch of entries.
    ///
    /// All updates are applied in a single transaction. If any update fails,
    /// no changes are committed (all-or-nothing).
    async fn upsert_batch(&self, entries: &[JournalEntry]) -> Result<(), JournalError>;

    /// Drop all entries for a pair (called when deleting a sync pair).
    async fn clear_pair(&self, pair_id: &PairId) -> Result<(), JournalError>;

    // ── Conflict persistence ─────────────────────────────────────────────────

    /// Insert or update a conflict record.
    async fn upsert_conflict(&self, record: &ConflictRecord) -> Result<(), JournalError>;

    /// Mark a conflict as resolved.
    async fn resolve_conflict(
        &self,
        id: &str,
        resolution: ConflictResolution,
    ) -> Result<(), JournalError>;

    /// Return all conflict records for a pair (resolved + unresolved).
    async fn list_conflicts(&self, pair_id: &PairId) -> Result<Vec<ConflictRecord>, JournalError>;
}
