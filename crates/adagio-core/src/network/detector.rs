/// Platform-specific probe for network and power conditions.
///
/// All methods must return a safe default (`false` / `None`) when the
/// platform API is unavailable — never panic.
pub trait NetworkDetector: Send + Sync {
    /// Returns `true` if the active network connection is metered.
    fn is_metered(&self) -> bool;
    /// Returns `true` if the device is currently on battery (not AC).
    fn is_on_battery(&self) -> bool;
    /// Returns the current Wi-Fi SSID, or `None` if unknown.
    fn current_ssid(&self) -> Option<String>;
}

// ── MockNetworkDetector ───────────────────────────────────────────────────────

/// Test double for `NetworkDetector` with injectable state.
#[derive(Debug, Default)]
pub struct MockNetworkDetector {
    pub metered: bool,
    pub on_battery: bool,
    pub ssid: Option<String>,
}

impl NetworkDetector for MockNetworkDetector {
    fn is_metered(&self) -> bool {
        self.metered
    }
    fn is_on_battery(&self) -> bool {
        self.on_battery
    }
    fn current_ssid(&self) -> Option<String> {
        self.ssid.clone()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T005: MockNetworkDetector reports injected state.
    #[test]
    fn mock_detector_reports_injected_state() {
        let det = MockNetworkDetector {
            metered: true,
            on_battery: false,
            ssid: Some("HomeNet".to_string()),
        };
        assert!(det.is_metered());
        assert!(!det.is_on_battery());
        assert_eq!(det.current_ssid(), Some("HomeNet".to_string()));
    }

    #[test]
    fn mock_detector_default_is_all_false() {
        let det = MockNetworkDetector::default();
        assert!(!det.is_metered());
        assert!(!det.is_on_battery());
        assert_eq!(det.current_ssid(), None);
    }
}
