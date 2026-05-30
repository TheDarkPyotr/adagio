pub mod detector;
pub mod fallback;
pub mod monitor;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(windows)]
pub mod windows;

pub use detector::{MockNetworkDetector, NetworkDetector};
pub use monitor::NetworkMonitor;

use serde::{Deserialize, Serialize};

// ── NetworkAction ─────────────────────────────────────────────────────────────

/// What the sync engine should do when a network condition is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NetworkAction {
    /// No restriction — sync runs at the configured bandwidth limit.
    #[default]
    Allow,
    /// Apply `throttle_kbps` as an upload and download ceiling.
    Throttle,
    /// Halt all sync activity until the condition clears.
    Pause,
}

// ── NetworkPolicy ─────────────────────────────────────────────────────────────

/// Global network-awareness rules persisted in `config.json`.
///
/// All fields default to the most permissive value so existing configs
/// without a `network_policy` key continue to behave as before.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Action to take when the active connection is metered.
    #[serde(default)]
    pub on_metered: NetworkAction,
    /// Action to take when the device is on battery power.
    #[serde(default)]
    pub on_battery: NetworkAction,
    /// Shared Kbps ceiling used by both `Throttle` actions. 0 = unlimited.
    #[serde(default)]
    pub throttle_kbps: u64,
    /// Exact SSID names on which sync always pauses (case-sensitive).
    #[serde(default)]
    pub blocked_ssids: Vec<String>,
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            on_metered: NetworkAction::Allow,
            on_battery: NetworkAction::Allow,
            throttle_kbps: 0,
            blocked_ssids: Vec::new(),
        }
    }
}

// ── NetworkState ──────────────────────────────────────────────────────────────

/// Runtime snapshot of detected network and power conditions.
#[derive(Debug, Clone, Default)]
pub struct NetworkState {
    /// True when the active connection is reported as metered.
    pub metered: bool,
    /// True when the device is on battery (not AC).
    pub on_battery: bool,
    /// Current Wi-Fi SSID, or `None` if unknown / not on Wi-Fi.
    pub ssid: Option<String>,
}

// ── EffectiveAction ───────────────────────────────────────────────────────────

/// The resolved action derived from `NetworkPolicy` + `NetworkState`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveAction {
    pub action: NetworkAction,
    /// Kbps limit when `action == Throttle`; 0 otherwise.
    pub throttle_kbps: u64,
    /// Human-readable reason, empty when action is Allow.
    pub reason: String,
}

impl Default for EffectiveAction {
    fn default() -> Self {
        Self {
            action: NetworkAction::Allow,
            throttle_kbps: 0,
            reason: String::new(),
        }
    }
}

// ── Policy composition ────────────────────────────────────────────────────────

/// Derive the effective action from a policy and a live network state.
///
/// Priority (most restrictive wins):
/// 1. Blocked SSID → Pause
/// 2. Metered + on_metered==Pause → Pause
/// 3. Battery + on_battery==Pause → Pause
/// 4. Metered + on_metered==Throttle → Throttle
/// 5. Battery + on_battery==Throttle → Throttle
/// 6. → Allow
pub fn derive_effective_action(policy: &NetworkPolicy, state: &NetworkState) -> EffectiveAction {
    // Blocked SSID always wins.
    if let Some(ref ssid) = state.ssid {
        if policy.blocked_ssids.iter().any(|b| b == ssid) {
            return EffectiveAction {
                action: NetworkAction::Pause,
                throttle_kbps: 0,
                reason: format!("blocked SSID: {ssid}"),
            };
        }
    }

    // Pause conditions.
    if state.metered && policy.on_metered == NetworkAction::Pause {
        return EffectiveAction {
            action: NetworkAction::Pause,
            throttle_kbps: 0,
            reason: "metered connection".to_string(),
        };
    }
    if state.on_battery && policy.on_battery == NetworkAction::Pause {
        return EffectiveAction {
            action: NetworkAction::Pause,
            throttle_kbps: 0,
            reason: "on battery".to_string(),
        };
    }

    // Throttle conditions.
    if state.metered && policy.on_metered == NetworkAction::Throttle {
        return EffectiveAction {
            action: NetworkAction::Throttle,
            throttle_kbps: policy.throttle_kbps,
            reason: "metered connection".to_string(),
        };
    }
    if state.on_battery && policy.on_battery == NetworkAction::Throttle {
        return EffectiveAction {
            action: NetworkAction::Throttle,
            throttle_kbps: policy.throttle_kbps,
            reason: "on battery".to_string(),
        };
    }

    EffectiveAction::default()
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_policy() -> NetworkPolicy {
        NetworkPolicy::default()
    }

    // T004-a: Default policy allows everything.
    #[test]
    fn policy_default_is_all_allow() {
        let policy = NetworkPolicy::default();
        assert_eq!(policy.on_metered, NetworkAction::Allow);
        assert_eq!(policy.on_battery, NetworkAction::Allow);
        assert_eq!(policy.throttle_kbps, 0);
        assert!(policy.blocked_ssids.is_empty());
        let state = NetworkState::default();
        assert_eq!(
            derive_effective_action(&policy, &state).action,
            NetworkAction::Allow
        );
    }

    // T004-b: Blocked SSID always results in Pause regardless of other conditions.
    #[test]
    fn effective_action_blocked_ssid_always_wins() {
        let policy = NetworkPolicy {
            blocked_ssids: vec!["CoffeeShop-Guest".to_string()],
            on_metered: NetworkAction::Allow,
            on_battery: NetworkAction::Allow,
            ..Default::default()
        };
        let state = NetworkState {
            ssid: Some("CoffeeShop-Guest".to_string()),
            metered: false,
            on_battery: false,
        };
        let ea = derive_effective_action(&policy, &state);
        assert_eq!(ea.action, NetworkAction::Pause);
        assert!(ea.reason.contains("CoffeeShop-Guest"));
    }

    // T004-c: Blocked SSID wins over a Throttle policy.
    #[test]
    fn effective_action_blocked_ssid_beats_throttle() {
        let policy = NetworkPolicy {
            blocked_ssids: vec!["BadNet".to_string()],
            on_metered: NetworkAction::Throttle,
            throttle_kbps: 100,
            ..Default::default()
        };
        let state = NetworkState {
            ssid: Some("BadNet".to_string()),
            metered: true,
            on_battery: false,
        };
        assert_eq!(
            derive_effective_action(&policy, &state).action,
            NetworkAction::Pause
        );
    }

    // T004-d: Pause beats Throttle when both conditions apply.
    #[test]
    fn effective_action_pause_beats_throttle() {
        let policy = NetworkPolicy {
            on_metered: NetworkAction::Pause,
            on_battery: NetworkAction::Throttle,
            throttle_kbps: 500,
            ..Default::default()
        };
        let state = NetworkState {
            metered: true,
            on_battery: true,
            ssid: None,
        };
        assert_eq!(
            derive_effective_action(&policy, &state).action,
            NetworkAction::Pause
        );
    }

    // T004-e: Allow when no active conditions match.
    #[test]
    fn effective_action_allow_when_no_conditions_match() {
        let policy = NetworkPolicy {
            on_metered: NetworkAction::Pause,
            on_battery: NetworkAction::Throttle,
            throttle_kbps: 200,
            blocked_ssids: vec!["BlockedNet".to_string()],
            ..Default::default()
        };
        // Not metered, not on battery, different SSID.
        let state = NetworkState {
            metered: false,
            on_battery: false,
            ssid: Some("HomeNetwork".to_string()),
        };
        assert_eq!(
            derive_effective_action(&policy, &state).action,
            NetworkAction::Allow
        );
    }

    // T004-f: Throttle from battery condition.
    #[test]
    fn effective_action_throttle_from_battery() {
        let policy = NetworkPolicy {
            on_battery: NetworkAction::Throttle,
            throttle_kbps: 200,
            ..Default::default()
        };
        let state = NetworkState {
            on_battery: true,
            ..Default::default()
        };
        let ea = derive_effective_action(&allow_policy(), &NetworkState::default());
        assert_eq!(ea.action, NetworkAction::Allow);

        let ea2 = derive_effective_action(&policy, &state);
        assert_eq!(ea2.action, NetworkAction::Throttle);
        assert_eq!(ea2.throttle_kbps, 200);
        assert_eq!(ea2.reason, "on battery");
    }
}
