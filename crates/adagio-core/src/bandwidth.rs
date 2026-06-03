use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Duration;

/// A token-bucket rate limiter (per-direction).
///
/// Adds `rate` tokens (bytes) per second, capped at `burst`.
/// `acquire(n)` returns the Duration the caller must sleep before sending `n` bytes.
/// `rate == 0` means unlimited: acquire always returns `Duration::ZERO`.
#[allow(missing_debug_implementations)]
pub struct TokenBucket {
    rate: u64,
    burst: u64,
    available: Arc<Mutex<f64>>,
    last_refill: Arc<Mutex<std::time::Instant>>,
}

impl TokenBucket {
    pub fn new(rate: u64, burst: u64) -> Self {
        Self {
            rate,
            burst,
            available: Arc::new(Mutex::new(burst as f64)),
            last_refill: Arc::new(Mutex::new(std::time::Instant::now())),
        }
    }

    pub fn unlimited() -> Self {
        Self::new(0, u64::MAX)
    }

    /// Consume `n` tokens. Returns the Duration the caller should sleep before proceeding.
    pub async fn acquire(&self, n: u64) -> Duration {
        if self.rate == 0 {
            return Duration::ZERO;
        }

        let mut available = self.available.lock().await;
        let mut last_refill = self.last_refill.lock().await;

        let now = std::time::Instant::now();
        let elapsed = now.duration_since(*last_refill).as_secs_f64();
        *last_refill = now;

        let refill = elapsed * self.rate as f64;
        *available = (*available + refill).min(self.burst as f64);

        let needed = n as f64;
        if *available >= needed {
            *available -= needed;
            Duration::ZERO
        } else {
            let deficit = needed - *available;
            *available = 0.0;
            Duration::from_secs_f64(deficit / self.rate as f64)
        }
    }
}

// ── Bandwidth schedule ────────────────────────────────────────────────────────

/// A single time window in a bandwidth schedule.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TimeWindow {
    /// Inclusive start hour (0–23).
    pub start_hour: u8,
    /// Exclusive end hour (0–23).
    pub end_hour: u8,
    /// Bandwidth cap in bytes/sec; 0 = unlimited.
    pub cap_bytes_per_sec: u64,
}

/// A collection of time windows mapping hours of day to bandwidth caps.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BandwidthSchedule {
    pub windows: Vec<TimeWindow>,
}

impl BandwidthSchedule {
    /// Returns the cap for `hour` (0–23), or `None` if no window matches (= unlimited).
    pub fn cap_at_hour(&self, hour: u8) -> Option<u64> {
        self.windows
            .iter()
            .find(|w| w.start_hour <= hour && hour < w.end_hour)
            .map(|w| w.cap_bytes_per_sec)
    }
}

// ── ThroughputMeter ───────────────────────────────────────────────────────────

/// Rolling-window byte-rate calculator.
///
/// Tracks bytes transferred over a configurable sliding window (default 10 s)
/// and computes an instantaneous rate in bytes/second.
pub struct ThroughputMeter {
    window: std::collections::VecDeque<(std::time::Instant, u64)>,
    window_secs: u64,
}

impl ThroughputMeter {
    /// Create a new meter with the default 10-second window.
    pub fn new() -> Self {
        Self {
            window: std::collections::VecDeque::new(),
            window_secs: 10,
        }
    }

    /// Record that `bytes` bytes were transferred at this instant.
    pub fn record(&mut self, bytes: u64) {
        self.window.push_back((std::time::Instant::now(), bytes));
        self.evict();
    }

    /// Return the current rate in bytes/second (rolling window average).
    pub fn rate_bytes_per_sec(&mut self) -> u64 {
        self.evict();
        let total: u64 = self.window.iter().map(|(_, b)| b).sum();
        if total == 0 {
            return 0;
        }
        let elapsed_secs = self
            .window
            .front()
            .map(|(t, _)| t.elapsed().as_secs_f64())
            .unwrap_or(0.0);
        if elapsed_secs < 0.001 {
            return total; // avoid division by near-zero
        }
        (total as f64 / elapsed_secs) as u64
    }

    fn evict(&mut self) {
        let cutoff = std::time::Duration::from_secs(self.window_secs);
        while self
            .window
            .front()
            .map(|(t, _)| t.elapsed() > cutoff)
            .unwrap_or(false)
        {
            self.window.pop_front();
        }
    }
}

impl Default for ThroughputMeter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests (T063) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T063-1: unlimited bucket never makes callers wait.
    #[tokio::test]
    async fn token_bucket_unlimited_never_waits() {
        let bucket = TokenBucket::unlimited();
        let delay = bucket.acquire(1024 * 1024).await;
        assert_eq!(
            delay,
            Duration::ZERO,
            "unlimited bucket should return zero delay"
        );
    }

    // T063-2: burst capacity is fully available on first acquire.
    #[tokio::test]
    async fn token_bucket_burst_available_immediately() {
        let bucket = TokenBucket::new(1024, 4096); // 1 KB/s, 4 KB burst
        let delay = bucket.acquire(4096).await;
        assert_eq!(
            delay,
            Duration::ZERO,
            "burst capacity should be available immediately"
        );
    }

    // T063-3: acquiring more than available tokens returns a positive delay.
    #[tokio::test]
    async fn token_bucket_waits_when_depleted() {
        let bucket = TokenBucket::new(1024, 1024); // 1 KB/s, 1 KB burst
        bucket.acquire(1024).await; // exhaust burst
        let delay = bucket.acquire(512).await;
        assert!(delay > Duration::ZERO, "should wait when bucket is empty");
        // At 1 KB/s, 512 bytes → ~500 ms
        assert!(
            delay >= Duration::from_millis(400),
            "delay should be at least 400 ms"
        );
        assert!(
            delay <= Duration::from_millis(600),
            "delay should be at most 600 ms"
        );
    }

    // T063-4: bucket refills proportionally to elapsed time.
    #[tokio::test]
    async fn token_bucket_refills_over_time() {
        let bucket = TokenBucket::new(1024, 1024); // 1 KB/s, 1 KB burst
        bucket.acquire(1024).await; // exhaust burst
        tokio::time::sleep(Duration::from_millis(500)).await;
        // After 500 ms at 1 KB/s, ~512 bytes have refilled; acquiring 256 should be free.
        let delay = bucket.acquire(256).await;
        assert_eq!(
            delay,
            Duration::ZERO,
            "bucket should have refilled after 500 ms"
        );
    }

    // T063-5: schedule returns the cap for the matching time window.
    #[test]
    fn bandwidth_schedule_returns_cap_for_matching_window() {
        let schedule = BandwidthSchedule {
            windows: vec![
                TimeWindow {
                    start_hour: 9,
                    end_hour: 17,
                    cap_bytes_per_sec: 512 * 1024,
                },
                TimeWindow {
                    start_hour: 0,
                    end_hour: 9,
                    cap_bytes_per_sec: 0,
                },
            ],
        };
        assert_eq!(schedule.cap_at_hour(10), Some(512 * 1024));
        assert_eq!(schedule.cap_at_hour(5), Some(0));
        assert_eq!(schedule.cap_at_hour(18), None);
    }

    // T063-6: schedule with no windows always returns None.
    #[test]
    fn bandwidth_schedule_empty_returns_none() {
        let schedule = BandwidthSchedule::default();
        assert_eq!(schedule.cap_at_hour(12), None);
    }

    // T002-a: New meter returns zero rate.
    #[test]
    fn throughput_meter_empty_returns_zero() {
        let mut m = ThroughputMeter::new();
        assert_eq!(m.rate_bytes_per_sec(), 0);
    }

    // T002-b: After recording bytes, rate is non-zero.
    #[test]
    fn throughput_meter_records_bytes() {
        let mut m = ThroughputMeter::new();
        m.record(100_000);
        // Rate should be non-zero immediately after recording.
        assert!(m.rate_bytes_per_sec() > 0);
    }

    // T002-c: Entries older than window_secs are evicted.
    #[test]
    fn throughput_meter_evicts_old_entries() {
        let mut m = ThroughputMeter {
            window: std::collections::VecDeque::new(),
            window_secs: 0, // zero-length window → all entries immediately stale
        };
        m.record(999_999);
        // After eviction with 0s window, meter should be empty.
        assert_eq!(m.rate_bytes_per_sec(), 0);
    }
}
