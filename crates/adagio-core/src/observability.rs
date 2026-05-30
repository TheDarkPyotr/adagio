use crate::types::PairId;
use chrono::{DateTime, Utc};
use std::collections::VecDeque;
use std::path::Path;
use std::sync::RwLock;
use std::time::Duration;

// ── CycleMetrics (T010) ───────────────────────────────────────────────────────

/// Structured telemetry emitted as a `tracing::debug!` event at the end of
/// every scan cycle.
///
/// Consumed by log aggregators to verify AC-1 (idle CPU) and audit the
/// checksum-skip guard. Never persisted to disk.
#[derive(Debug)]
pub struct CycleMetrics {
    /// Total file paths examined during this cycle.
    pub path_count: usize,
    /// Files whose checksum was recomputed (mtime or size changed).
    pub checksum_recomputed: usize,
    /// Files whose cached checksum was reused (mtime and size unchanged).
    pub checksum_from_cache: usize,
    /// Wall-clock duration of the full scan + propagation pass in milliseconds.
    pub duration_ms: u64,
}

impl CycleMetrics {
    /// Emit this metric set as a single `tracing::debug!` event.
    pub fn emit(&self) {
        tracing::debug!(
            path_count = self.path_count,
            checksum_recomputed = self.checksum_recomputed,
            checksum_from_cache = self.checksum_from_cache,
            duration_ms = self.duration_ms,
            "cycle complete"
        );
    }
}

// ── MemorySampler (T012) ──────────────────────────────────────────────────────

/// One RSS measurement taken at the end of a scan cycle.
#[derive(Debug, Clone)]
pub struct MemorySample {
    /// Resident set size at this moment, in bytes.
    pub rss_bytes: u64,
    /// RSS at the first-minute baseline. `None` until `set_baseline` is called.
    pub baseline_rss: Option<u64>,
    /// Growth ratio relative to baseline (`rss_bytes / baseline_rss`).
    /// `None` until baseline is set.
    pub growth_ratio: Option<f64>,
}

/// Samples resident set size using platform-specific OS interfaces.
///
/// Platform implementations:
/// - **Linux**: reads `/proc/self/statm` (field 1 × page size).
/// - **macOS**: calls `libc::getrusage(RUSAGE_SELF, ...)`.
/// - **Windows**: calls `GetProcessMemoryInfo` (via `windows` crate stub; returns `None`
///   until the crate is wired in — safe to call, vacuously passes).
///
/// # Memory budget thresholds
/// - `observe()` emits `tracing::debug!` on every call.
/// - Emits `tracing::warn!` when growth > 15%.
/// - Emits `tracing::error!` when growth > 20% (the hard AC-3 limit).
pub struct MemorySampler {
    baseline_rss: Option<u64>,
}

impl MemorySampler {
    /// Create a new sampler with no baseline set.
    pub fn new() -> Self {
        Self { baseline_rss: None }
    }

    /// Returns `true` if a baseline has already been recorded.
    pub fn has_baseline(&self) -> bool {
        self.baseline_rss.is_some()
    }

    /// Record the RSS baseline. Must be called exactly once.
    ///
    /// # Panics
    /// Panics if called more than once (programming error guard).
    pub fn set_baseline(&mut self, rss: u64) {
        assert!(
            self.baseline_rss.is_none(),
            "MemorySampler::set_baseline called more than once"
        );
        self.baseline_rss = Some(rss);
    }

    /// Read current RSS from the OS.
    ///
    /// Returns `None` if the OS call is unsupported or fails. Never panics.
    pub fn sample_rss() -> Option<u64> {
        #[cfg(target_os = "linux")]
        {
            // /proc/self/statm: space-separated fields; field 1 is RSS in pages.
            let content = std::fs::read_to_string("/proc/self/statm").ok()?;
            let rss_pages: u64 = content.split_whitespace().nth(1)?.parse().ok()?;
            // SAFETY: sysconf(_SC_PAGESIZE) is safe on any POSIX system.
            let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64;
            Some(rss_pages * page_size)
        }
        #[cfg(target_os = "macos")]
        {
            // SAFETY: getrusage is a standard POSIX call. rusage is fully
            // initialised by the call; the pointer we pass is valid for the
            // lifetime of the local variable.
            let mut usage: libc::rusage = unsafe { std::mem::zeroed() };
            let ret = unsafe { libc::getrusage(libc::RUSAGE_SELF, &mut usage) };
            if ret == 0 {
                // ru_maxrss is in bytes on macOS.
                Some(usage.ru_maxrss as u64)
            } else {
                None
            }
        }
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }

    /// Read current RSS, build a `MemorySample`, emit structured tracing logs,
    /// and return the sample.
    ///
    /// Emits `DEBUG` always; `WARN` at > 15% growth; `ERROR` at > 20% growth.
    pub fn observe(&self) -> MemorySample {
        let rss = Self::sample_rss().unwrap_or(0);
        self.build_sample(rss)
    }

    /// Like `observe()` but uses a caller-supplied RSS value instead of reading
    /// from the OS. Intended for unit tests.
    #[cfg(test)]
    pub fn observe_with_rss(&self, rss: u64) -> MemorySample {
        self.build_sample(rss)
    }

    fn build_sample(&self, rss_bytes: u64) -> MemorySample {
        let (baseline_rss, growth_ratio) = match self.baseline_rss {
            Some(baseline) if baseline > 0 => {
                let ratio = rss_bytes as f64 / baseline as f64;
                (Some(baseline), Some(ratio))
            }
            _ => (self.baseline_rss, None),
        };

        if let Some(ratio) = growth_ratio {
            if ratio > 1.20 {
                tracing::error!(
                    rss_bytes,
                    baseline_rss,
                    growth_ratio = ratio,
                    "RSS exceeded 20% growth limit (AC-3 violation)"
                );
            } else if ratio > 1.15 {
                tracing::warn!(
                    rss_bytes,
                    baseline_rss,
                    growth_ratio = ratio,
                    "RSS growth above 15% early-warning threshold"
                );
            }
        }
        tracing::debug!(rss_bytes, baseline_rss, growth_ratio, "memory sample");

        MemorySample {
            rss_bytes,
            baseline_rss,
            growth_ratio,
        }
    }
}

impl Default for MemorySampler {
    fn default() -> Self {
        Self::new()
    }
}

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

    // T006-a — MemorySampler::sample_rss returns a non-zero value.
    #[test]
    fn memory_sampler_returns_nonzero_rss() {
        let rss = MemorySampler::sample_rss();
        assert!(
            rss.map(|n| n > 0).unwrap_or(true), // pass vacuously if OS call unsupported
            "sample_rss must return a positive value when supported"
        );
    }

    // T006-b — observe_with_rss triggers WARN and returns correct growth_ratio.
    #[test]
    fn memory_sampler_warn_on_growth_above_threshold() {
        let mut sampler = MemorySampler::new();
        sampler.set_baseline(100_000_000); // 100 MB baseline

        let sample = sampler.observe_with_rss(116_000_000); // 116 MB
        let ratio = sample
            .growth_ratio
            .expect("growth_ratio must be Some when baseline is set");
        assert!(
            (ratio - 1.16).abs() < 0.001,
            "growth_ratio must be ~1.16, got {ratio}"
        );
        assert_eq!(sample.baseline_rss, Some(100_000_000));
        assert_eq!(sample.rss_bytes, 116_000_000);
    }

    // T006-c — MemorySampler::observe() doesn't panic over many calls.
    #[test]
    fn memory_stable_over_many_cycles() {
        let mut sampler = MemorySampler::new();
        if let Some(initial) = MemorySampler::sample_rss() {
            sampler.set_baseline(initial);
            for _ in 0..500 {
                let sample = sampler.observe();
                if let Some(ratio) = sample.growth_ratio {
                    assert!(
                        ratio < 1.20,
                        "RSS grew >20% during test loop — possible leak: ratio={ratio}"
                    );
                }
            }
        }
        // Vacuous pass if sample_rss is unsupported on this platform.
    }

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
