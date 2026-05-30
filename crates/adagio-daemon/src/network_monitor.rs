use adagio_core::cycle::DefaultSyncEngine;
use adagio_core::network::{NetworkMonitor, NetworkPolicy};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tokio_util::task::TaskTracker;
use tracing::info;

/// Build the platform-specific `NetworkDetector`, create a `NetworkMonitor`,
/// spawn its poll loop into the `TaskTracker`, and return the monitor handle.
///
/// The returned `Arc<NetworkMonitor>` is stored in `DaemonProcess` so the
/// dispatcher can read live `NetworkState` and update `NetworkPolicy`.
pub fn spawn_network_monitor(
    engine: Arc<DefaultSyncEngine>,
    policy: Arc<RwLock<NetworkPolicy>>,
    tracker: &TaskTracker,
    cancel: CancellationToken,
) -> Arc<NetworkMonitor> {
    let detector = build_detector();
    let monitor = Arc::new(NetworkMonitor::new(detector, policy, engine));
    let monitor_clone = monitor.clone();
    tracker.spawn(async move {
        monitor_clone.run(cancel).await;
    });
    info!("NetworkMonitor spawned");
    monitor
}

/// Returns the platform-appropriate `NetworkDetector`.
fn build_detector() -> Arc<dyn adagio_core::network::NetworkDetector> {
    #[cfg(target_os = "linux")]
    return Arc::new(adagio_core::network::linux::LinuxDetector);

    #[cfg(target_os = "macos")]
    return Arc::new(adagio_core::network::macos::MacosDetector);

    #[cfg(windows)]
    return Arc::new(adagio_core::network::windows::WindowsDetector);

    #[allow(unreachable_code)]
    Arc::new(adagio_core::network::fallback::FallbackDetector)
}
