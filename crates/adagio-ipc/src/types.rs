use serde::{Deserialize, Serialize};

// ── Request (client → daemon) ─────────────────────────────────────────────────

/// Every RPC method the daemon accepts.
///
/// Serialises as `{"method": "<snake_case>", "params": { ... }}`.
/// The outer `id` field is added by the `DaemonClient` wrapper, not here.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "snake_case")]
pub enum DaemonRequest {
    // ── System ────────────────────────────────────────────────────────────────
    /// Health-check: returns version + uptime. Used for single-instance guard.
    Ping,
    /// Request graceful shutdown of the daemon (30 s drain).
    StopDaemon,
    /// Register or deregister the daemon binary for OS login auto-start.
    SetStartAtLogin { enabled: bool },

    // ── Sync control ─────────────────────────────────────────────────────────
    /// Return the current engine-level sync status.
    GetStatus,
    /// Trigger an immediate sync cycle for the given pair.
    TriggerSync { pair_id: String },
    /// Pause all sync activity.
    PauseSyncAll,
    /// Resume sync after a pause.
    ResumeSyncAll,
    /// Return the last `limit` activity log entries, optionally filtered by kind.
    GetActivityLog {
        limit: Option<u32>,
        filter: Option<String>,
    },
    /// Return items currently in error state.
    GetErrorItems { pair_id: String },

    // ── Pairs ─────────────────────────────────────────────────────────────────
    /// Return all configured sync pairs.
    ListPairs,
    /// Create a new sync pair.
    CreatePair {
        account_id: String,
        local_root: String,
        remote_root: String,
    },
    /// Delete a sync pair, optionally deleting local files.
    DeletePair {
        pair_id: String,
        delete_local_files: bool,
    },
    /// Return the synced file tree for a pair at the given path.
    ListSyncedFiles {
        pair_id: String,
        relative_path: Option<String>,
    },
    /// Return exclude patterns.
    GetExcludePatterns,
    /// Return the remote tree for a pair.
    ListRemoteTree { pair_id: String },

    // ── Accounts ──────────────────────────────────────────────────────────────
    /// Return all configured accounts.
    ListAccounts,
    /// Add an account with known credentials.
    AddAccount {
        server_url: String,
        username: String,
        display_name: String,
        secret: String,
    },
    /// Remove an account and all associated pairs.
    RemoveAccount { account_id: String },
    /// Run the OAuth2 browser flow and return the resulting account.
    ConnectAccountOAuth2 { server_url: String },

    // ── Conflicts ─────────────────────────────────────────────────────────────
    /// Return all conflict records for a pair.
    ListConflicts { pair_id: String },
    /// Resolve a conflict by choosing which version to keep.
    ResolveConflict { id: String, side: String },
    /// Dismiss all pending conflicts without file I/O.
    DismissAllConflicts,

    // ── Sharing ───────────────────────────────────────────────────────────────
    /// Search for Nextcloud users matching a query.
    SearchUsers { account_id: String, query: String },
    /// Create a share for a file or folder.
    CreateShare {
        account_id: String,
        path: String,
        recipients: serde_json::Value,
        expiry_date: Option<String>,
        link_password: Option<String>,
        hide_download: bool,
        notify_on_open: bool,
        note: Option<String>,
    },

    /// Subscription marker — sent on the subscription connection to identify it.
    Subscribe,

    // ── Bandwidth throttling ──────────────────────────────────────────────────
    /// Return current bandwidth limits and live throughput for an account.
    /// `account_id: None` applies to the first account.
    GetBandwidthStatus { account_id: Option<String> },

    /// Set upload and/or download speed limits in Kbps (0 = unlimited).
    SetBandwidthLimits {
        account_id: Option<String>,
        upload_kbps: u64,
        download_kbps: u64,
    },

    /// Remove all bandwidth limits for an account (equivalent to set both to 0).
    ClearBandwidthLimits { account_id: Option<String> },
}

// ── Response (daemon → client) ────────────────────────────────────────────────

/// The successful result payload for each `DaemonRequest`.
///
/// Serialised as the `"result"` field inside `{"id": N, "result": ...}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DaemonResponse {
    Pong {
        version: String,
        uptime_secs: u64,
    },
    Unit {},
    Status(serde_json::Value),
    Pairs(Vec<serde_json::Value>),
    Pair(serde_json::Value),
    Accounts(Vec<serde_json::Value>),
    Account(serde_json::Value),
    SyncedFiles(Vec<serde_json::Value>),
    Strings(Vec<String>),
    RemoteTree(Vec<serde_json::Value>),
    ActivityLog(Vec<serde_json::Value>),
    ErrorItems(Vec<serde_json::Value>),
    Conflicts(Vec<serde_json::Value>),
    DismissedCount {
        dismissed_count: usize,
    },
    Users(Vec<serde_json::Value>),
    Share(serde_json::Value),
    DaemonStatus {
        running: bool,
        uptime_secs: Option<u64>,
        connection_state: String,
    },
}

// ── Push events (daemon → all subscribers) ────────────────────────────────────

/// A push notification emitted by the daemon to all subscription connections.
///
/// Serialised as `{"event": "<snake_case>", "payload": { ... }}\n`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", content = "payload", rename_all = "snake_case")]
pub enum DaemonEvent {
    /// A new Ask-policy conflict was detected.
    ConflictDetected { pending_count: usize },
    /// A conflict was resolved or dismissed.
    ConflictResolved { id: String, pending_count: usize },
    /// Engine-level status changed.
    SyncStatusChanged {
        status: String,
        pair_id: Option<String>,
    },
    /// Per-file transfer progress.
    TransferProgress {
        pair_id: String,
        path: String,
        bytes_done: u64,
        bytes_total: u64,
    },
    /// Daemon is about to shut down.
    ShuttingDown { reason: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    // T005 — DaemonRequest::GetStatus serialises to snake_case method name.
    #[test]
    fn daemon_request_get_status_serialises_to_snake_case() {
        let req = DaemonRequest::GetStatus;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"].as_str().unwrap(), "get_status");
    }

    // T006 — DaemonEvent::ConflictDetected deserialises from NDJSON line.
    #[test]
    fn daemon_event_conflict_detected_deserialises() {
        let line = r#"{"event":"conflict_detected","payload":{"pending_count":2}}"#;
        let event: DaemonEvent = serde_json::from_str(line).unwrap();
        match event {
            DaemonEvent::ConflictDetected { pending_count } => {
                assert_eq!(pending_count, 2);
            }
            other => panic!("unexpected variant: {:?}", other),
        }
    }

    #[test]
    fn daemon_event_conflict_detected_serialises() {
        let event = DaemonEvent::ConflictDetected { pending_count: 3 };
        let json = serde_json::to_value(&event).unwrap();
        assert_eq!(json["event"].as_str().unwrap(), "conflict_detected");
        assert_eq!(json["payload"]["pending_count"].as_u64().unwrap(), 3);
    }

    #[test]
    fn daemon_request_ping_serialises() {
        let req = DaemonRequest::Ping;
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"].as_str().unwrap(), "ping");
    }

    #[test]
    fn daemon_request_resolve_conflict_serialises() {
        let req = DaemonRequest::ResolveConflict {
            id: "abc".to_string(),
            side: "local".to_string(),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["method"].as_str().unwrap(), "resolve_conflict");
        assert_eq!(json["params"]["id"].as_str().unwrap(), "abc");
    }
}
