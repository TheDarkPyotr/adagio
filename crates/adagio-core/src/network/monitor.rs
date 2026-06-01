use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info};

use crate::cycle::{DefaultSyncEngine, SyncEngine as _};

use super::{
    derive_effective_action, EffectiveAction, NetworkAction, NetworkDetector, NetworkPolicy,
    NetworkState,
};

/// Background task that monitors network and power conditions and adjusts
/// the sync engine according to the configured `NetworkPolicy`.
///
/// Runs a 3-second poll loop. On each tick it:
/// 1. Probes conditions via `NetworkDetector`
/// 2. Derives `EffectiveAction` from policy + state
/// 3. If the action changed, calls `pause()` or `resume()` on the engine
/// 4. Updates the shared `NetworkState` for status queries
///
/// Throttle bandwidth limits are set when `SetNetworkPolicy` is processed
/// by the IPC dispatcher, which calls `engine.update_bandwidth_limits` directly.
pub struct NetworkMonitor {
    detector: Arc<dyn NetworkDetector>,
    policy: Arc<RwLock<NetworkPolicy>>,
    engine: Arc<DefaultSyncEngine>,
    /// Shared live state exposed to IPC status queries.
    pub state: Arc<Mutex<NetworkState>>,
    /// True when the user manually paused sync; prevents auto-resume.
    user_paused: Arc<std::sync::Mutex<bool>>,
}

impl NetworkMonitor {
    /// Create a new monitor. Call [`Self::run`] to start the poll loop.
    pub fn new(
        detector: Arc<dyn NetworkDetector>,
        policy: Arc<RwLock<NetworkPolicy>>,
        engine: Arc<DefaultSyncEngine>,
    ) -> Self {
        Self {
            detector,
            policy,
            engine,
            state: Arc::new(Mutex::new(NetworkState::default())),
            user_paused: Arc::new(std::sync::Mutex::new(false)),
        }
    }

    /// Notify the monitor that the user manually paused or resumed sync.
    ///
    /// When `paused` is true the monitor will not auto-resume even when
    /// network conditions improve.
    pub fn set_user_paused(&self, paused: bool) {
        if let Ok(mut flag) = self.user_paused.lock() {
            *flag = paused;
        }
    }

    /// Run the monitoring loop until the cancellation token is cancelled.
    pub async fn run(self: Arc<Self>, token: CancellationToken) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(3));
        let mut last_action = EffectiveAction::default();

        info!("NetworkMonitor started");

        loop {
            tokio::select! {
                _ = token.cancelled() => {
                    info!("NetworkMonitor stopping");
                    break;
                }
                _ = interval.tick() => {
                    self.poll(&mut last_action).await;
                }
            }
        }
    }

    /// Execute one monitoring tick. Exposed for testing.
    pub async fn poll(&self, last_action: &mut EffectiveAction) {
        let new_state = NetworkState {
            metered: self.detector.is_metered(),
            on_battery: self.detector.is_on_battery(),
            ssid: self.detector.current_ssid(),
        };

        let policy = self.policy.read().await;
        let new_action = derive_effective_action(&policy, &new_state);
        drop(policy);

        // Update shared state for IPC queries.
        *self.state.lock().await = new_state;

        // Only act if the effective action changed.
        if new_action == *last_action {
            return;
        }

        debug!(
            action = ?new_action.action,
            reason = %new_action.reason,
            "NetworkMonitor: effective action changed"
        );

        let user_paused = self.user_paused.lock().map(|g| *g).unwrap_or(false);

        match new_action.action {
            NetworkAction::Pause => {
                let _ = self.engine.pause().await;
            }
            NetworkAction::Throttle | NetworkAction::Allow => {
                // Resume if we were paused by network policy (but not if user manually paused).
                if last_action.action == NetworkAction::Pause && !user_paused {
                    let _ = self.engine.resume().await;
                }
                // Note: actual Kbps throttle limits are applied by the IPC dispatcher
                // when SetNetworkPolicy is called (it calls engine.update_bandwidth_limits
                // per account directly).
            }
        }

        *last_action = new_action;
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::network::detector::MockNetworkDetector;

    fn make_monitor(det: MockNetworkDetector, policy: NetworkPolicy) -> Arc<NetworkMonitor> {
        Arc::new(NetworkMonitor::new(
            Arc::new(det),
            Arc::new(RwLock::new(policy)),
            Arc::new(DefaultSyncEngine::new()),
        ))
    }

    // T010-a: Monitor sets action to Pause when metered + on_metered=Pause.
    #[tokio::test]
    async fn monitor_pauses_engine_on_metered_pause_policy() {
        let monitor = make_monitor(
            MockNetworkDetector {
                metered: true,
                ..Default::default()
            },
            NetworkPolicy {
                on_metered: NetworkAction::Pause,
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Pause);
    }

    // T010-b: Monitor sets action to Throttle when metered + on_metered=Throttle.
    #[tokio::test]
    async fn monitor_throttles_engine_on_metered_throttle_policy() {
        let monitor = make_monitor(
            MockNetworkDetector {
                metered: true,
                ..Default::default()
            },
            NetworkPolicy {
                on_metered: NetworkAction::Throttle,
                throttle_kbps: 200,
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Throttle);
        assert_eq!(last.throttle_kbps, 200);
    }

    // T011: Monitor does not re-fire when state is unchanged (deduplication).
    #[tokio::test]
    async fn monitor_deduplicates_state_calls() {
        let monitor = make_monitor(
            MockNetworkDetector {
                metered: true,
                ..Default::default()
            },
            NetworkPolicy {
                on_metered: NetworkAction::Pause,
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await; // → Pause
        let snapshot = last.action;
        monitor.poll(&mut last).await; // unchanged state → last_action stays
                                       // last_action should still be Pause; no spurious re-firing.
        assert_eq!(snapshot, NetworkAction::Pause);
        assert_eq!(last.action, NetworkAction::Pause);
    }

    // T024-a: Battery + on_battery=Pause → action is Pause.
    #[tokio::test]
    async fn monitor_pauses_engine_on_battery_pause_policy() {
        let monitor = make_monitor(
            MockNetworkDetector {
                on_battery: true,
                ..Default::default()
            },
            NetworkPolicy {
                on_battery: NetworkAction::Pause,
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Pause);
        assert_eq!(last.reason, "on battery");
    }

    // T024-b: Battery + on_battery=Throttle → action is Throttle.
    #[tokio::test]
    async fn monitor_throttles_engine_on_battery_throttle_policy() {
        let monitor = make_monitor(
            MockNetworkDetector {
                on_battery: true,
                ..Default::default()
            },
            NetworkPolicy {
                on_battery: NetworkAction::Throttle,
                throttle_kbps: 500,
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Throttle);
        assert_eq!(last.throttle_kbps, 500);
    }

    // T025: AC restored → action becomes Allow.
    #[tokio::test]
    async fn monitor_allows_when_ac_restored() {
        let monitor = make_monitor(
            MockNetworkDetector {
                on_battery: false,
                ..Default::default()
            },
            NetworkPolicy {
                on_battery: NetworkAction::Pause,
                ..Default::default()
            },
        );
        // Simulate previous state was Pause.
        let mut last = EffectiveAction {
            action: NetworkAction::Pause,
            ..Default::default()
        };
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Allow);
    }

    // T031-a: Blocked SSID → action is Pause.
    #[tokio::test]
    async fn monitor_pauses_on_blocked_ssid_connection() {
        let monitor = make_monitor(
            MockNetworkDetector {
                ssid: Some("CoffeeShop-Guest".to_string()),
                ..Default::default()
            },
            NetworkPolicy {
                blocked_ssids: vec!["CoffeeShop-Guest".to_string()],
                ..Default::default()
            },
        );
        let mut last = EffectiveAction::default();
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Pause);
        assert!(last.reason.contains("CoffeeShop-Guest"));
    }

    // T031-b: Disconnecting from blocked SSID → action becomes Allow.
    #[tokio::test]
    async fn monitor_resumes_on_blocked_ssid_disconnect() {
        let policy_arc = Arc::new(RwLock::new(NetworkPolicy {
            blocked_ssids: vec!["BadNet".to_string()],
            ..Default::default()
        }));
        let monitor = Arc::new(NetworkMonitor::new(
            Arc::new(MockNetworkDetector {
                ssid: Some("OtherNet".to_string()),
                ..Default::default()
            }),
            policy_arc,
            Arc::new(DefaultSyncEngine::new()),
        ));
        let mut last = EffectiveAction {
            action: NetworkAction::Pause,
            reason: "blocked SSID: BadNet".to_string(),
            ..Default::default()
        };
        monitor.poll(&mut last).await;
        assert_eq!(last.action, NetworkAction::Allow);
    }
}
