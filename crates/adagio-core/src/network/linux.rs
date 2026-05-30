use super::detector::NetworkDetector;

/// Linux network detector using NetworkManager D-Bus and UPower D-Bus.
///
/// All D-Bus calls fail gracefully — any error returns the safe default.
#[derive(Debug, Default)]
pub struct LinuxDetector;

impl NetworkDetector for LinuxDetector {
    fn is_metered(&self) -> bool {
        is_metered_via_nm().unwrap_or(false)
    }

    fn is_on_battery(&self) -> bool {
        is_on_battery_via_upower()
            .or_else(|| is_on_battery_via_sysfs())
            .unwrap_or(false)
    }

    fn current_ssid(&self) -> Option<String> {
        ssid_via_nm().ok().flatten()
    }
}

// ── Metered detection ─────────────────────────────────────────────────────────

/// Query NetworkManager for the metered status of the primary connection.
///
/// Returns `Ok(true)` when the NM `Metered` property is 2 (yes) or 3 (yes-guessed).
/// Returns `Ok(false)` for 0 (unknown), 1 (no), or any other value.
/// Returns `Err` if D-Bus is unavailable; caller should treat as false.
fn is_metered_via_nm() -> Result<bool, ()> {
    // Use a blocking D-Bus call so this can be called from a sync context.
    // The monitor runs this on a Tokio blocking thread (spawn_blocking).
    let conn = zbus::blocking::Connection::system().map_err(|_| ())?;

    // Get the primary active connection path.
    let nm_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .map_err(|_| ())?;

    let primary_conn: zbus::zvariant::OwnedObjectPath =
        nm_proxy.get_property("PrimaryConnection").map_err(|_| ())?;

    if primary_conn.as_str() == "/" {
        return Ok(false); // No active connection.
    }

    // Get the device object path via the active connection.
    let ac_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        primary_conn.as_str(),
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .map_err(|_| ())?;

    let devices: Vec<zbus::zvariant::OwnedObjectPath> =
        ac_proxy.get_property("Devices").map_err(|_| ())?;

    let device_path = devices.into_iter().next().ok_or(())?;

    let dev_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        device_path.as_str(),
        "org.freedesktop.NetworkManager.Device",
    )
    .map_err(|_| ())?;

    let metered: u32 = dev_proxy.get_property("Metered").map_err(|_| ())?;
    // 2 = NM_METERED_YES, 3 = NM_METERED_GUESS_YES
    Ok(metered == 2 || metered == 3)
}

// ── Battery detection ─────────────────────────────────────────────────────────

/// Query UPower D-Bus for the aggregate power state.
///
/// Returns `Some(true)` when the display device state is 2 (discharging).
fn is_on_battery_via_upower() -> Option<bool> {
    let conn = zbus::blocking::Connection::system().ok()?;
    let proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower/devices/DisplayDevice",
        "org.freedesktop.UPower.Device",
    )
    .ok()?;
    let state: u32 = proxy.get_property("State").ok()?;
    Some(state == 2) // 2 = UP_DEVICE_STATE_DISCHARGING
}

/// Fallback battery detection via sysfs.
fn is_on_battery_via_sysfs() -> Option<bool> {
    for name in &["BAT0", "BAT1", "battery"] {
        let status_path = format!("/sys/class/power_supply/{name}/status");
        if let Ok(status) = std::fs::read_to_string(&status_path) {
            let s = status.trim().to_lowercase();
            return Some(s == "discharging");
        }
    }
    None
}

// ── SSID detection ────────────────────────────────────────────────────────────

/// Query NetworkManager for the SSID of the active Wi-Fi access point.
fn ssid_via_nm() -> Result<Option<String>, ()> {
    let conn = zbus::blocking::Connection::system().map_err(|_| ())?;

    let nm_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        "/org/freedesktop/NetworkManager",
        "org.freedesktop.NetworkManager",
    )
    .map_err(|_| ())?;

    let primary_conn: zbus::zvariant::OwnedObjectPath =
        nm_proxy.get_property("PrimaryConnection").map_err(|_| ())?;

    if primary_conn.as_str() == "/" {
        return Ok(None);
    }

    let ac_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        primary_conn.as_str(),
        "org.freedesktop.NetworkManager.Connection.Active",
    )
    .map_err(|_| ())?;

    let devices: Vec<zbus::zvariant::OwnedObjectPath> =
        ac_proxy.get_property("Devices").map_err(|_| ())?;

    let device_path = match devices.into_iter().next() {
        Some(p) => p,
        None => return Ok(None),
    };

    // Check if this is a wireless device.
    let wireless_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        device_path.as_str(),
        "org.freedesktop.NetworkManager.Device.Wireless",
    )
    .map_err(|_| ())?;

    let ap_path: zbus::zvariant::OwnedObjectPath = wireless_proxy
        .get_property("ActiveAccessPoint")
        .map_err(|_| ())?;

    if ap_path.as_str() == "/" {
        return Ok(None);
    }

    let ap_proxy = zbus::blocking::Proxy::new(
        &conn,
        "org.freedesktop.NetworkManager",
        ap_path.as_str(),
        "org.freedesktop.NetworkManager.AccessPoint",
    )
    .map_err(|_| ())?;

    let ssid_bytes: Vec<u8> = ap_proxy.get_property("Ssid").map_err(|_| ())?;
    Ok(String::from_utf8(ssid_bytes).ok())
}
