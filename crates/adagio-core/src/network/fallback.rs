use super::detector::NetworkDetector;

/// Safe no-op detector for platforms where APIs are unavailable.
///
/// Always returns `false` / `None` — sync proceeds without restriction.
#[derive(Debug, Default)]
pub struct FallbackDetector;

impl NetworkDetector for FallbackDetector {
    fn is_metered(&self) -> bool {
        false
    }
    fn is_on_battery(&self) -> bool {
        false
    }
    fn current_ssid(&self) -> Option<String> {
        None
    }
}
