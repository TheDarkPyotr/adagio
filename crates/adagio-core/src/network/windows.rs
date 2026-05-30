use super::detector::NetworkDetector;

/// Windows network detector.
///
/// - **Metered**: `INetworkListManager` via `windows-rs`.
/// - **Battery**: `battery` crate (WMI).
/// - **SSID**: `netsh wlan show interfaces` subprocess.
#[derive(Debug, Default)]
pub struct WindowsDetector;

impl NetworkDetector for WindowsDetector {
    fn is_metered(&self) -> bool {
        is_metered_via_nlm().unwrap_or(false)
    }

    fn is_on_battery(&self) -> bool {
        battery_state().unwrap_or(false)
    }

    fn current_ssid(&self) -> Option<String> {
        ssid_via_netsh()
    }
}

fn is_metered_via_nlm() -> Option<bool> {
    use windows::Win32::Networking::NetworkListManager::{
        INetworkConnectionCost, INetworkListManager, NetworkListManager, NLM_CONNECTION_COST_FIXED,
        NLM_CONNECTION_COST_ROAMING, NLM_CONNECTION_COST_VARIABLE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL};

    unsafe {
        let manager: INetworkListManager =
            CoCreateInstance(&NetworkListManager, None, CLSCTX_ALL).ok()?;
        let connections = manager.GetNetworkConnections().ok()?;
        for conn in connections {
            let conn = conn.ok()?;
            if let Ok(cost) = conn.cast::<INetworkConnectionCost>() {
                if let Ok(flags) = cost.GetCost() {
                    if flags
                        & (NLM_CONNECTION_COST_FIXED
                            | NLM_CONNECTION_COST_VARIABLE
                            | NLM_CONNECTION_COST_ROAMING)
                        != 0
                    {
                        return Some(true);
                    }
                }
            }
        }
        Some(false)
    }
}

fn battery_state() -> Option<bool> {
    let manager = battery::Manager::new().ok()?;
    let mut batteries = manager.batteries().ok()?;
    let bat = batteries.next()?.ok()?;
    Some(bat.state() == battery::State::Discharging)
}

/// Parse the current SSID from `netsh wlan show interfaces`.
fn ssid_via_netsh() -> Option<String> {
    let output = std::process::Command::new("netsh")
        .args(["wlan", "show", "interfaces"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    for line in text.lines() {
        let trimmed = line.trim();
        // Matches "    SSID                   : HomeNetwork"
        if let Some(rest) = trimmed.strip_prefix("SSID") {
            if let Some(ssid) = rest
                .trim_start_matches(|c: char| c == ' ' || c == ':')
                .trim()
                .into()
            {
                if !ssid.is_empty() {
                    return Some(ssid.to_string());
                }
            }
        }
    }
    None
}
