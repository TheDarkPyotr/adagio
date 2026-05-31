use thiserror::Error;

// ── Exponential-backoff retry policy (T073/T075) ──────────────────────────────

/// Policy for exponential-backoff retries.
///
/// Default: 1 s base, 2× doubling, 5 min cap, ±10 % jitter, 5 attempts max.
#[derive(Debug, Clone)]
pub struct BackoffPolicy {
    /// Delay for attempt 0 in milliseconds. Default: 1 000.
    pub base_ms: u64,
    /// Multiplication factor per attempt. Default: 2.0.
    pub multiplier: f64,
    /// Maximum delay in milliseconds. Default: 300 000 (5 min).
    pub cap_ms: u64,
    /// Fractional random jitter applied symmetrically: ±(fraction × delay).
    /// Set to 0.0 for fully deterministic delays (useful in tests).
    pub jitter_fraction: f64,
    /// Park the item (stop retrying) after this many attempts. Default: 5.
    pub max_attempts: u32,
}

impl Default for BackoffPolicy {
    fn default() -> Self {
        Self {
            base_ms: 1_000,
            multiplier: 2.0,
            cap_ms: 300_000,
            jitter_fraction: 0.1,
            max_attempts: 5,
        }
    }
}

impl BackoffPolicy {
    /// Returns the sleep `Duration` for `attempt` (0-indexed).
    ///
    /// Returns `None` when `attempt >= max_attempts` — the caller should park
    /// the item and surface it in the error list.
    pub fn backoff_for(&self, attempt: u32) -> Option<std::time::Duration> {
        if attempt >= self.max_attempts {
            return None;
        }
        let base =
            (self.base_ms as f64 * self.multiplier.powi(attempt as i32)).min(self.cap_ms as f64);
        let jitter = if self.jitter_fraction > 0.0 {
            base * self.jitter_fraction * pseudo_jitter()
        } else {
            0.0
        };
        Some(std::time::Duration::from_millis(
            (base + jitter).max(1.0) as u64
        ))
    }
}

fn pseudo_jitter() -> f64 {
    use std::time::SystemTime;
    let nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    (nanos as f64 / u32::MAX as f64) * 2.0 - 1.0 // -1.0 … +1.0
}

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("journal error: {0}")]
    Journal(#[from] JournalError),

    #[error("transfer error: {0}")]
    Transfer(#[from] TransferError),

    #[error("detection error: {0}")]
    Detection(#[from] DetectorError),

    #[error("remote client error: {0}")]
    Remote(#[from] ClientError),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("fatal: {0} — journal rebuild required")]
    Fatal(String),

    #[error("sync pair not found: {0}")]
    PairNotFound(String),

    #[error("account not found: {0}")]
    AccountNotFound(String),

    #[error("permanent error: {0}")]
    Permanent(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum JournalError {
    #[error("corruption: {0}")]
    Corruption(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("constraint violation: {0}")]
    Constraint(String),

    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
}

#[derive(Debug, Error)]
pub enum TransferError {
    /// Retry-eligible: network, server 5xx, timeout.
    #[error("transient: {0}")]
    Transient(String),

    /// Do not retry: checksum mismatch (repeated), disk full, permission denied.
    #[error("permanent: {0}")]
    Permanent(String),

    /// File is being written by another process; defer to next cycle.
    #[error("file in progress")]
    FileInProgress,
}

#[derive(Debug, Error)]
pub enum DetectorError {
    #[error("filesystem error: {0}")]
    Fs(#[from] std::io::Error),

    #[error("watch error: {0}")]
    Watch(String),

    #[error("remote error: {0}")]
    Remote(String),

    /// File is being written by another process; skip and retry next cycle.
    #[error("file in progress")]
    FileInProgress,
}

#[derive(Debug, Error)]
pub enum ClientError {
    /// Retry-eligible: network, server 5xx, timeout.
    #[error("transient: {0}")]
    Transient(String),

    /// Do not retry: 400, 403, 404, checksum mismatch.
    #[error("permanent: {0}")]
    Permanent(String),

    /// Session expired or credentials invalid.
    #[error("auth required")]
    AuthRequired,

    /// Server is in maintenance mode (HTTP 503). Sync will resume automatically.
    #[error("server in maintenance mode")]
    Maintenance,
}

// ── Tests (T073) ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn no_jitter() -> BackoffPolicy {
        BackoffPolicy {
            jitter_fraction: 0.0,
            ..Default::default()
        }
    }

    // T073-1: attempt 0 returns base_ms (1 s).
    #[test]
    fn backoff_attempt_0_is_base() {
        let d = no_jitter().backoff_for(0).expect("should have delay");
        assert_eq!(d, Duration::from_millis(1_000));
    }

    // T073-2: each attempt doubles.
    #[test]
    fn backoff_doubles_each_attempt() {
        let p = no_jitter();
        assert_eq!(p.backoff_for(0).unwrap(), Duration::from_millis(1_000));
        assert_eq!(p.backoff_for(1).unwrap(), Duration::from_millis(2_000));
        assert_eq!(p.backoff_for(2).unwrap(), Duration::from_millis(4_000));
        assert_eq!(p.backoff_for(3).unwrap(), Duration::from_millis(8_000));
    }

    // T073-3: cap is enforced regardless of attempt.
    #[test]
    fn backoff_capped_at_max() {
        // max_attempts must exceed the attempt index being tested.
        let p = BackoffPolicy {
            cap_ms: 5_000,
            jitter_fraction: 0.0,
            max_attempts: 20,
            ..Default::default()
        };
        // 2^4 × 1000 = 16_000 ms, capped at 5_000
        assert_eq!(p.backoff_for(4).unwrap(), Duration::from_millis(5_000));
    }

    // T073-4: returns None after max_attempts.
    #[test]
    fn backoff_parks_after_max_attempts() {
        let p = no_jitter();
        assert!(
            p.backoff_for(5).is_none(),
            "attempt == max_attempts should park"
        );
        assert!(
            p.backoff_for(99).is_none(),
            "attempt > max_attempts should park"
        );
    }

    // T073-5: jitter keeps delay within ±fraction of the exact value.
    #[test]
    fn backoff_jitter_within_bounds() {
        let p = BackoffPolicy {
            jitter_fraction: 0.1,
            ..Default::default()
        };
        let d = p.backoff_for(0).unwrap().as_millis() as f64;
        assert!(
            d >= 900.0,
            "delay should be at least 900 ms with ±10 % jitter"
        );
        assert!(
            d <= 1_100.0,
            "delay should be at most 1100 ms with ±10 % jitter"
        );
    }

    // T073-6: custom max_attempts = 3 parks at attempt 3.
    #[test]
    fn backoff_custom_max_attempts() {
        let p = BackoffPolicy {
            max_attempts: 3,
            jitter_fraction: 0.0,
            ..Default::default()
        };
        assert!(p.backoff_for(2).is_some());
        assert!(p.backoff_for(3).is_none());
    }
}
