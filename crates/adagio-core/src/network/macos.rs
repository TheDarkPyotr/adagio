use super::detector::NetworkDetector;

/// macOS network detector.
///
/// - **Metered**: No public API on macOS — always returns `false`.
///   Users can use the SSID block list as a workaround.
/// - **Battery**: `battery` crate (IOKit).
/// - **SSID**: `networksetup -getairportnetwork <interface>` subprocess.
#[derive(Debug, Default)]
pub struct MacosDetector;

impl NetworkDetector for MacosDetector {
    fn is_metered(&self) -> bool {
        false
    }

    fn is_on_battery(&self) -> bool {
        battery_state().unwrap_or(false)
    }

    fn current_ssid(&self) -> Option<String> {
        ssid_via_networksetup()
    }
}

fn battery_state() -> Option<bool> {
    let manager = battery::Manager::new().ok()?;
    let mut batteries = manager.batteries().ok()?;
    let bat = batteries.next()?.ok()?;
    Some(bat.state() == battery::State::Discharging)
}

/// Run `networksetup -getairportnetwork <interface>` and parse the SSID.
///
/// Returns `None` if Wi-Fi is off, no interface found, or the command fails.
fn ssid_via_networksetup() -> Option<String> {
    // Find the first Wi-Fi service interface (typically "en0").
    let interfaces = std::process::Command::new("networksetup")
        .args(["-listallhardwareports"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&interfaces.stdout);
    let iface = parse_wifi_interface(&text)?;

    let output = std::process::Command::new("networksetup")
        .args(["-getairportnetwork", &iface])
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&output.stdout);
    // Output: "Current Wi-Fi Network: <ssid>" or "You are not associated with an AirPort network."
    let prefix = "Current Wi-Fi Network: ";
    let ssid = line.trim().strip_prefix(prefix)?;
    Some(ssid.to_string())
}

fn parse_wifi_interface(hardware_ports: &str) -> Option<String> {
    let mut found_wifi = false;
    for line in hardware_ports.lines() {
        let line = line.trim();
        if line.contains("Wi-Fi") || line.contains("AirPort") {
            found_wifi = true;
        }
        if found_wifi {
            if let Some(rest) = line.strip_prefix("Device:") {
                return Some(rest.trim().to_string());
            }
        }
    }
    None
}
