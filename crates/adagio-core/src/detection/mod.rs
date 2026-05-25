use crate::error::DetectorError;
use crate::journal::Journal;
use crate::remote::RemoteClient;
use crate::types::{LocalItem, PairId, RemoteItem, SyncPair};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use tokio::sync::watch;

pub mod exclusion;
pub mod local;
pub mod remote;

/// Signals that one or more local changes have occurred and stabilized.
#[derive(Debug, Clone)]
pub struct LocalChangeSignal {
    pub pair_id: PairId,
    pub detected_at: DateTime<Utc>,
}

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
    ///
    /// Idempotent per pair: calling it twice for the same pair returns receivers
    /// backed by the same underlying watcher.
    fn subscribe_local_events(&self, pair: &SyncPair) -> watch::Receiver<LocalChangeSignal>;
}
