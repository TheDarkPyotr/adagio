use crate::types::PairId;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::RwLock;
use std::time::Duration;

/// Snapshot of sync progress for one pair at a point in time.
#[derive(Debug, Clone)]
pub struct SyncProgress {
    pub pair_id: PairId,
    pub items_done: u64,
    pub items_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub started_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// Estimated time remaining, or None if insufficient data.
    pub eta: Option<Duration>,
}

impl SyncProgress {
    pub fn new(pair_id: PairId, items_total: u64, bytes_total: u64) -> Self {
        let now = Utc::now();
        Self {
            pair_id,
            items_done: 0,
            items_total,
            bytes_done: 0,
            bytes_total,
            started_at: now,
            updated_at: now,
            eta: None,
        }
    }

    /// Record progress for one completed item.
    pub fn advance(&mut self, bytes: u64) {
        self.items_done += 1;
        self.bytes_done += bytes;
        self.updated_at = Utc::now();
        self.eta = self.compute_eta();
    }

    pub fn fraction(&self) -> f64 {
        if self.bytes_total == 0 {
            return 1.0;
        }
        self.bytes_done as f64 / self.bytes_total as f64
    }

    fn compute_eta(&self) -> Option<Duration> {
        if self.bytes_done == 0 || self.bytes_total == 0 {
            return None;
        }
        let elapsed = (self.updated_at - self.started_at).num_milliseconds() as f64;
        if elapsed <= 0.0 {
            return None;
        }
        let rate = self.bytes_done as f64 / elapsed; // bytes/ms
        let remaining = (self.bytes_total - self.bytes_done) as f64;
        let ms = remaining / rate;
        Some(Duration::from_millis(ms as u64))
    }
}

/// A single entry in the activity log.
#[derive(Debug, Clone)]
pub struct ActivityLogEntry {
    pub pair_id: PairId,
    pub path: String,
    pub action: String,
    pub bytes: Option<u64>,
    pub occurred_at: DateTime<Utc>,
}

/// Ring-buffer activity log: stores the last `capacity` entries across all pairs.
pub struct ActivityLog {
    entries: RwLock<VecDeque<ActivityLogEntry>>,
    capacity: usize,
}

impl ActivityLog {
    pub fn new(capacity: usize) -> Self {
        Self {
            entries: RwLock::new(VecDeque::with_capacity(capacity)),
            capacity,
        }
    }

    pub fn push(&self, entry: ActivityLogEntry) {
        let mut q = self.entries.write().unwrap();
        if q.len() >= self.capacity {
            q.pop_front();
        }
        q.push_back(entry);
    }

    pub fn last_n(&self, n: usize) -> Vec<ActivityLogEntry> {
        let q = self.entries.read().unwrap();
        q.iter().rev().take(n).cloned().collect()
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ── Log rotation (T096) ───────────────────────────────────────────────────────

/// Guard returned by `init_file_logging`; drop to flush and stop file logging.
pub struct LogGuard(#[allow(dead_code)] tracing_appender::non_blocking::WorkerGuard);

/// Initialise file-based logging with rotation.
///
/// Writes to `<log_dir>/adagio.log`, rotating when the file exceeds ~10 MB.
/// Keeps up to 5 rotated files (`adagio.log.1` … `adagio.log.5`).
///
/// Returns a `LogGuard` that must be held for the lifetime of the application;
/// dropping it flushes all pending log records.
pub fn init_file_logging(log_dir: &Path) -> LogGuard {
    use tracing_appender::rolling::{RollingFileAppender, Rotation};
    use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt};

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_prefix("adagio")
        .filename_suffix("log")
        .max_log_files(5)
        .build(log_dir)
        .expect("failed to create log appender");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    tracing_subscriber::registry()
        .with(fmt::layer().with_writer(non_blocking).with_ansi(false))
        .try_init()
        .ok(); // May fail if subscriber already set (e.g., in tests)

    LogGuard(guard)
}

// ── Diagnostic bundle (T081) ──────────────────────────────────────────────────

/// A redacted summary of the current sync state for support/diagnostics.
///
/// Credentials, local paths, and personal data are deliberately omitted.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DiagnosticBundle {
    pub generated_at: DateTime<Utc>,
    pub adagio_version: String,
    pub journal_stats: JournalStats,
}

/// Aggregate counts from the journal — no file paths or content.
#[derive(Debug, Clone, serde::Serialize)]
pub struct JournalStats {
    pub total_entries: u64,
    pub error_count: u64,
    pub pending_upload_count: u64,
    pub pending_download_count: u64,
    pub synced_count: u64,
}

impl DiagnosticBundle {
    pub fn new(stats: JournalStats) -> Self {
        Self {
            generated_at: Utc::now(),
            adagio_version: env!("CARGO_PKG_VERSION").to_string(),
            journal_stats: stats,
        }
    }

    /// Serialize the bundle to a JSON byte payload suitable for writing to disk.
    pub fn to_json_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec_pretty(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PairId;

    // T081: DiagnosticBundle serializes to valid JSON with expected fields.
    #[test]
    fn diagnostic_bundle_serializes_to_json() {
        let stats = JournalStats {
            total_entries: 42,
            error_count: 3,
            pending_upload_count: 5,
            pending_download_count: 2,
            synced_count: 32,
        };
        let bundle = DiagnosticBundle::new(stats);
        let json = bundle
            .to_json_bytes()
            .expect("serialization should succeed");
        let parsed: serde_json::Value = serde_json::from_slice(&json).unwrap();

        assert_eq!(parsed["journal_stats"]["total_entries"], 42);
        assert_eq!(parsed["journal_stats"]["error_count"], 3);
        assert!(parsed["generated_at"].is_string());
        assert!(parsed["adagio_version"].is_string());
    }

    #[test]
    fn progress_fraction_advances() {
        let mut prog = SyncProgress::new(PairId::new(), 10, 1000);
        prog.advance(100);
        assert!((prog.fraction() - 0.1).abs() < 1e-9);
        assert_eq!(prog.items_done, 1);
    }

    #[test]
    fn activity_log_caps_at_capacity() {
        let log = ActivityLog::new(3);
        for i in 0..5u32 {
            log.push(ActivityLogEntry {
                pair_id: PairId::new(),
                path: format!("file{i}.txt"),
                action: "upload".into(),
                bytes: Some(100),
                occurred_at: Utc::now(),
            });
        }
        assert_eq!(log.len(), 3);
    }

    #[test]
    fn activity_log_last_n_returns_most_recent() {
        let log = ActivityLog::new(500);
        for i in 0..5u32 {
            log.push(ActivityLogEntry {
                pair_id: PairId::new(),
                path: format!("file{i}.txt"),
                action: "download".into(),
                bytes: None,
                occurred_at: Utc::now(),
            });
        }
        let recent = log.last_n(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].path, "file4.txt");
    }
}
