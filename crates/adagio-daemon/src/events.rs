use adagio_ipc::DaemonEvent;
use tokio::sync::broadcast;

/// Broadcasts `DaemonEvent` push notifications to all connected subscription clients.
///
/// Clone the sender to share it between the dispatcher and the server accept loop.
/// Each connected subscription connection holds a `broadcast::Receiver<DaemonEvent>`.
#[derive(Clone)]
pub struct EventBroadcaster {
    tx: broadcast::Sender<DaemonEvent>,
}

impl EventBroadcaster {
    /// Create a new broadcaster with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Subscribe to the event stream. Returns a receiver that delivers a copy of
    /// every event emitted after this call. Lagging receivers receive
    /// `RecvError::Lagged` and lose events — acceptable for a UI update stream.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.tx.subscribe()
    }

    /// Emit `conflict_detected`.
    #[allow(dead_code)]
    pub fn emit_conflict_detected(&self, pending_count: usize) {
        let _ = self
            .tx
            .send(DaemonEvent::ConflictDetected { pending_count });
    }

    /// Emit `conflict_resolved`.
    pub fn emit_conflict_resolved(&self, id: String, pending_count: usize) {
        let _ = self
            .tx
            .send(DaemonEvent::ConflictResolved { id, pending_count });
    }

    /// Emit `sync_status_changed`.
    #[allow(dead_code)]
    pub fn emit_sync_status_changed(&self, status: String, pair_id: Option<String>) {
        let _ = self
            .tx
            .send(DaemonEvent::SyncStatusChanged { status, pair_id });
    }

    /// Emit `transfer_progress`.
    #[allow(dead_code)]
    pub fn emit_transfer_progress(
        &self,
        pair_id: String,
        path: String,
        bytes_done: u64,
        bytes_total: u64,
    ) {
        let _ = self.tx.send(DaemonEvent::TransferProgress {
            pair_id,
            path,
            bytes_done,
            bytes_total,
        });
    }

    /// Emit `shutting_down` before the process exits.
    pub fn emit_shutting_down(&self, reason: String) {
        let _ = self.tx.send(DaemonEvent::ShuttingDown { reason });
    }
}
