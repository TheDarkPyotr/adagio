use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::{info, warn};

const DRAIN_TIMEOUT: Duration = Duration::from_secs(30);

/// Cancel all running tasks and wait up to 30 seconds for them to finish.
///
/// After the timeout, any remaining tasks are abandoned (they will be killed
/// when the process exits). The SQLite WAL checkpoint is the caller's
/// responsibility — call `journal.checkpoint()` before dropping the pool.
pub async fn graceful_shutdown(cancel: CancellationToken, tracker: TaskTracker) {
    info!("graceful shutdown: cancelling all tasks");
    cancel.cancel();
    tracker.close();

    match tokio::time::timeout(DRAIN_TIMEOUT, tracker.wait()).await {
        Ok(()) => {
            info!("graceful shutdown: all tasks drained");
        }
        Err(_) => {
            warn!("graceful shutdown: 30 s timeout exceeded — abandoning remaining tasks");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T013 — graceful_shutdown drains all tasks within the timeout.
    #[tokio::test]
    async fn graceful_shutdown_drains_tasks_within_30s() {
        let cancel = CancellationToken::new();
        let tracker = TaskTracker::new();

        // Spawn 3 short tasks (100 ms each, but they exit on cancellation).
        for _ in 0..3 {
            let token = cancel.clone();
            tracker.spawn(async move {
                tokio::select! {
                    _ = token.cancelled() => {}
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            });
        }

        graceful_shutdown(cancel, tracker).await;
        // If we reach here without panic or 30 s delay, the test passes.
    }
}
