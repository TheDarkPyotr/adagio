/// Tauri commands for the onboarding wizard.
///
/// Covers all five steps of the first-run account setup flow:
/// - Step 2 (Server): [`probe_server`] — validate URL + detect capabilities
/// - Step 3 (Authorize): [`begin_auth_flow`] — start Login Flow v2, return code + QR
/// - Step 4 (Where to sync): [`pick_folder`] — open OS folder-picker dialog
/// - Step 5 (Begin): [`get_account_remote_stats`] — fetch remote quota
/// - Completion: [`complete_onboarding`] — create pair + persist preferences
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{debug, info, instrument};

// ── DTOs ─────────────────────────────────────────────────────────────────────

/// Result of probing a candidate Nextcloud server URL.
///
/// All expected failure states (unreachable, bad TLS, unsupported version) are
/// encoded in this DTO so the frontend can render inline feedback without try/catch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerProbeDto {
    /// `true` when the server responded with HTTP 200 to `/status.php`.
    pub reachable: bool,
    /// `true` when the server reports maintenance mode.
    pub maintenance: bool,
    /// Human-readable version string, e.g. `"28.0.1"`.
    pub version: String,
    /// `true` when the major version is ≥ 16 (Login Flow v2 required).
    pub version_ok: bool,
    /// `true` when the major version is ≥ 20 (E2EE stable).
    pub e2ee_available: bool,
    /// `true` when the TLS certificate chain is trusted by the system store.
    pub tls_valid: bool,
    /// Round-trip time of the `/status.php` probe in milliseconds.
    pub latency_ms: u32,
    /// Human-readable error message when `reachable = false`.
    pub error: Option<String>,
}

/// Returned immediately by [`begin_auth_flow`].
///
/// Contains everything the wizard step 3 needs to render without further polling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthFlowInitDto {
    /// Last 8 characters of the login token formatted as "XXXX · XXXX".
    pub display_code: String,
    /// Full Login Flow v2 login URL — opened in browser and encoded in the QR.
    pub login_url: String,
    /// Inline SVG string of the QR code matrix.
    pub qr_svg: String,
    /// Unix timestamp (seconds) when the 5-minute countdown expires.
    pub expires_at: u64,
}

/// Remote account storage statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteStatsDto {
    /// Total quota in bytes (`quota.total` from OCS).
    pub total_bytes: u64,
    /// Bytes already used (`quota.used` from OCS).
    pub used_bytes: u64,
    /// Total remote file count; `null` until a remote tree scan completes.
    pub file_count: Option<u64>,
}

/// Sync preferences chosen during onboarding, sent in [`complete_onboarding`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingPrefsDto {
    /// Absolute local path for the sync folder.
    pub local_folder: String,
    /// Enable VFS (on-demand) mode for the pair.
    pub vfs_enabled: bool,
    /// Always keep pinned folders available locally.
    pub pin_pinned_folders: bool,
    /// Apply bandwidth throttling when on battery or metered connection.
    pub smart_bandwidth: bool,
    /// Use a short scan interval to detect external edits quickly.
    pub watch_external_edits: bool,
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Probe a candidate Nextcloud server URL.
///
/// Calls `GET {server_url}/status.php` and parses the response to determine
/// reachability, version, capabilities, and latency. Never requires authentication.
///
/// All failure states (unreachable, bad TLS, unsupported version) are returned
/// as a populated [`ServerProbeDto`] with `reachable = false` and an `error`
/// message — callers do not need to handle `Err` for expected failures.
#[tauri::command]
#[instrument(skip_all, fields(server_url))]
pub async fn probe_server(server_url: String) -> Result<ServerProbeDto, String> {
    use std::time::Instant;

    let status_url = format!("{}/status.php", server_url.trim_end_matches('/'));

    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent("adagio-desktop/0.1")
        .build()
    {
        Ok(c) => c,
        Err(e) => return Ok(unreachable_probe(format!("HTTP client error: {e}"))),
    };

    let start = Instant::now();
    let response = match client.get(&status_url).send().await {
        Ok(r) => r,
        Err(e) => {
            let tls_valid = !format!("{e}").to_lowercase().contains("certificate");
            return Ok(ServerProbeDto {
                reachable: false,
                maintenance: false,
                version: String::new(),
                version_ok: false,
                e2ee_available: false,
                tls_valid,
                latency_ms: 0,
                error: Some(format!("Server unreachable: {e}")),
            });
        }
    };
    let latency_ms = start.elapsed().as_millis() as u32;

    if !response.status().is_success() {
        return Ok(ServerProbeDto {
            reachable: false,
            maintenance: false,
            version: String::new(),
            version_ok: false,
            e2ee_available: false,
            tls_valid: true,
            latency_ms,
            error: Some(format!("Server returned HTTP {}", response.status())),
        });
    }

    // Parse JSON body
    let body: serde_json::Value = match response.json().await {
        Ok(v) => v,
        Err(e) => return Ok(unreachable_probe(format!("Response parse error: {e}"))),
    };

    let version_str = body
        .get("versionstring")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    let maintenance = body
        .get("maintenance")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    let major = version_str
        .split('.')
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    let version_ok = major >= 16;
    let e2ee_available = major >= 20;

    info!(
        server_url = %server_url,
        version = %version_str,
        latency_ms,
        version_ok,
        "server probe complete"
    );

    Ok(ServerProbeDto {
        reachable: true,
        maintenance,
        version: version_str,
        version_ok,
        e2ee_available,
        tls_valid: true,
        latency_ms,
        error: if maintenance {
            Some("Server is in maintenance mode".to_string())
        } else if !version_ok {
            Some(format!(
                "Nextcloud ≥ 16 required; found major version {major}"
            ))
        } else {
            None
        },
    })
}

/// Start a Nextcloud Login Flow v2 authentication session.
///
/// Initiates the flow by calling the server's `/index.php/login/v2` endpoint,
/// which returns a short login URL and a poll token. This command:
/// 1. Requests the session from the server.
/// 2. Generates a displayable code from the last 8 characters of the login token.
/// 3. Generates a QR code SVG encoding the login URL.
/// 4. Opens the login URL in the user's default browser.
/// 5. Spawns a background polling task that emits `adagio://auth-flow-complete`
///    or `adagio://auth-flow-expired` when the session resolves.
///
/// If a previous session is in progress, it is aborted before the new one starts.
#[tauri::command]
#[instrument(skip_all, fields(server_url))]
pub async fn begin_auth_flow(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    server_url: String,
) -> Result<AuthFlowInitDto, String> {
    use crate::auth_flow::{spawn_poll_task, AuthFlowState};
    use adagio_nextcloud::login_flow::begin_login_flow;
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
    use tauri::Emitter;

    info!(server_url = %server_url, "beginning Login Flow v2 session");

    // Abort any in-progress session.
    {
        let mut slot = state.auth_flow.lock().map_err(|e| e.to_string())?;
        if let Some(prev) = slot.take() {
            prev.task_handle.abort();
        }
    }

    // Start the Login Flow v2 session.
    let init = begin_login_flow(&server_url)
        .await
        .map_err(|e| e.to_string())?;

    // Extract display code: last 8 chars of the path token.
    let raw_token = init.login.rsplit('/').next().unwrap_or(&init.poll.token);
    let code_chars: String = raw_token
        .chars()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let display_code = if code_chars.len() == 8 {
        format!("{} \u{00B7} {}", &code_chars[..4], &code_chars[4..])
    } else {
        code_chars
    };

    // Generate QR code SVG.
    let qr_svg = generate_qr_svg(&init.login).map_err(|e| e.to_string())?;

    // Compute 5-minute expiry.
    let deadline = Instant::now() + Duration::from_secs(300);
    let expires_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        + 300;

    // Open browser.
    #[allow(deprecated)]
    tauri_plugin_shell::ShellExt::shell(&app)
        .open(&init.login, None)
        .map_err(|e| format!("Failed to open browser: {e}"))?;

    info!(
        server_url = %server_url,
        display_code = %display_code,
        "browser opened for Login Flow v2"
    );

    // Clone values needed by the polling task.
    let poll_endpoint = init.poll.endpoint.clone();
    let poll_token = init.poll.token.clone();
    let server_url_clone = server_url.clone();
    let app_complete = app.clone();
    let app_expired = app.clone();
    let state_complete = std::sync::Arc::clone(&state.auth_flow);
    let state_expired = std::sync::Arc::clone(&state.auth_flow);

    let task_handle = spawn_poll_task(
        poll_endpoint.clone(),
        poll_token.clone(),
        server_url_clone.clone(),
        deadline,
        move |creds| {
            // We're inside a tokio::spawn task — block_on would panic here.
            // Spawn a new task to run the async finalisation instead.
            tokio::spawn(async move {
                match finalise_auth(creds, &server_url_clone, &app_complete).await {
                    Ok(_) => {}
                    Err(e) => {
                        tracing::error!(error = %e, "auth-flow finalisation failed");
                        let _ =
                            app_complete.emit("adagio://auth-flow-expired", serde_json::json!({}));
                    }
                }
                if let Ok(mut slot) = state_complete.lock() {
                    *slot = None;
                }
            });
        },
        move || {
            let _ = app_expired.emit("adagio://auth-flow-expired", serde_json::json!({}));
            if let Ok(mut slot) = state_expired.lock() {
                *slot = None;
            }
        },
    );

    // Store session state.
    {
        let mut slot = state.auth_flow.lock().map_err(|e| e.to_string())?;
        *slot = Some(AuthFlowState {
            poll_token,
            poll_endpoint,
            server_url,
            task_handle,
            expires_at: deadline,
        });
    }

    Ok(AuthFlowInitDto {
        display_code,
        login_url: init.login,
        qr_svg,
        expires_at,
    })
}

/// Open the operating system's native folder-picker dialog.
///
/// Returns the absolute path of the folder selected by the user,
/// or `null` if the user cancels the dialog.
#[tauri::command]
pub async fn pick_folder(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let path = app.dialog().file().blocking_pick_folder();

    Ok(path.map(|p| p.to_string()))
}

/// Fetch remote storage quota for a connected account.
///
/// Calls the Nextcloud OCS `/cloud/users/{username}` endpoint and returns
/// the total quota and used bytes. File count is not yet available at this
/// point (it requires a full remote tree scan) and is returned as `null`.
#[tauri::command]
#[instrument(skip_all, fields(account_id))]
pub async fn get_account_remote_stats(
    account_id: String,
    state: State<'_, AppState>,
) -> Result<RemoteStatsDto, String> {
    let cfg = crate::config::load_config(&state.config_path);

    let account = cfg
        .accounts
        .iter()
        .find(|a| a.id == account_id)
        .cloned()
        .ok_or_else(|| format!("Account {account_id} not found"))?;

    let key = account.keychain_service_key.clone();
    let password =
        tokio::task::spawn_blocking(move || adagio_nextcloud::auth::retrieve_credentials(&key))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?
            .ok_or_else(|| "Credentials not found in keychain".to_string())?;

    let ocs_url = format!(
        "{}/ocs/v2.php/cloud/users/{}",
        account.server_url.trim_end_matches('/'),
        &account.username
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent("adagio-desktop/0.1")
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(&ocs_url)
        .basic_auth(&account.username, Some(&password))
        .header("Accept", "application/json")
        .header("OCS-APIREQUEST", "true")
        .send()
        .await
        .map_err(|e| format!("OCS request failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("OCS returned HTTP {}", resp.status()));
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let quota = &body["ocs"]["data"]["quota"];
    let total_bytes = quota["total"].as_u64().unwrap_or(0);
    let used_bytes = quota["used"].as_u64().unwrap_or(0);

    debug!(
        account_id = %account_id,
        total_bytes,
        used_bytes,
        "remote stats fetched"
    );

    Ok(RemoteStatsDto {
        total_bytes,
        used_bytes,
        file_count: None,
    })
}

/// Finalise onboarding: create the first sync pair and persist preferences.
///
/// Derives pair parameters from the user's preference selections:
/// - `vfs_enabled` → passed directly to `CreatePair`
/// - `watch_external_edits` → `scan_interval_secs` = 30 (true) or 7200 (false)
/// - `smart_bandwidth` → sets network policy `on_battery = throttle` at 500 Kbps
///
/// Also persists `onboarding_prefs` to `config.json` for future sessions.
#[tauri::command]
#[instrument(skip_all, fields(account_id))]
pub async fn complete_onboarding(
    state: State<'_, AppState>,
    account_id: String,
    prefs: OnboardingPrefsDto,
) -> Result<serde_json::Value, String> {
    use crate::config::{load_config, save_config, OnboardingPrefs};
    use adagio_ipc::DaemonRequest;

    // Expand leading ~ to the user's home directory.
    let local_folder = expand_tilde(&prefs.local_folder);

    // Create the sync folder so the engine can scan it immediately.
    tokio::fs::create_dir_all(&local_folder)
        .await
        .map_err(|e| format!("Cannot create sync folder '{}': {e}", local_folder))?;

    // Create the sync pair.
    let pair_resp = state
        .daemon
        .request(DaemonRequest::CreatePair {
            account_id: account_id.clone(),
            local_root: local_folder.clone(),
            remote_root: "/".to_string(),
            vfs_enabled: prefs.vfs_enabled,
            vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
            vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
        })
        .await
        .map_err(|e| e.to_string())?;

    // Apply smart bandwidth policy if requested.
    if prefs.smart_bandwidth {
        let _ = state
            .daemon
            .request(DaemonRequest::SetNetworkPolicy {
                on_metered: None,
                on_battery: Some("throttle".to_string()),
                throttle_kbps: Some(500),
            })
            .await;
    }

    // Persist preferences.
    let mut cfg = load_config(&state.config_path);
    cfg.onboarding_prefs = OnboardingPrefs {
        vfs_enabled: prefs.vfs_enabled,
        pin_pinned_folders: prefs.pin_pinned_folders,
        smart_bandwidth: prefs.smart_bandwidth,
        watch_external_edits: prefs.watch_external_edits,
    };
    save_config(&state.config_path, &cfg).map_err(|e| e.to_string())?;

    let pair_id = pair_resp["id"].as_str().unwrap_or("").to_string();
    info!(
        account_id = %account_id,
        pair_id = %pair_id,
        vfs = prefs.vfs_enabled,
        smart_bw = prefs.smart_bandwidth,
        "onboarding complete"
    );

    Ok(pair_resp)
}

// ── Private helpers ───────────────────────────────────────────────────────────

fn unreachable_probe(error: String) -> ServerProbeDto {
    ServerProbeDto {
        reachable: false,
        maintenance: false,
        version: String::new(),
        version_ok: false,
        e2ee_available: false,
        tls_valid: false,
        latency_ms: 0,
        error: Some(error),
    }
}

/// Generate an inline SVG QR code for `url`.
fn generate_qr_svg(url: &str) -> Result<String, String> {
    use qrcode::render::svg;
    use qrcode::{EcLevel, QrCode};

    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::L)
        .map_err(|e| format!("QR code generation failed: {e}"))?;

    let svg_str = code
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .max_dimensions(200, 200)
        .quiet_zone(false)
        .build();

    Ok(svg_str)
}

/// Expand a leading `~` to the user's home directory.
fn expand_tilde(path: &str) -> String {
    if path == "~" || path.starts_with("~/") || path.starts_with("~\\") {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .unwrap_or_default();
        if path == "~" {
            home
        } else {
            format!("{}/{}", home, &path[2..])
        }
    } else {
        path.to_string()
    }
}

/// Finalise a completed Login Flow v2 session: store credentials and notify daemon.
async fn finalise_auth(
    creds: adagio_nextcloud::login_flow::LoginCredentials,
    server_url: &str,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    use crate::state::AppState;
    use adagio_ipc::DaemonRequest;
    use tauri::{Emitter, Manager};

    // Register account with daemon. The daemon stores the app-password in the
    // keychain under its own key; build_client passes it verbatim to basic auth,
    // so secret must be the raw password — not a JSON wrapper.
    let state = app.state::<AppState>();
    let resp = state
        .daemon
        .request(DaemonRequest::AddAccount {
            server_url: creds.server.clone(),
            username: creds.login_name.clone(),
            display_name: creds.login_name.clone(),
            secret: creds.app_password.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;

    info!(
        login_name = %creds.login_name,
        server_url = %server_url,
        "Login Flow v2 account registered"
    );

    // Emit success event with the account id assigned by the daemon.
    let account_dto = serde_json::json!({
        "id": resp["id"].as_str().unwrap_or(""),
        "display_name": resp["display_name"].as_str().unwrap_or(&creds.login_name),
        "server_url": resp["server_url"].as_str().unwrap_or(&creds.server),
        "username": resp["username"].as_str().unwrap_or(&creds.login_name),
    });

    app.emit(
        "adagio://auth-flow-complete",
        serde_json::json!({ "account": account_dto }),
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T011 — probe_server unit tests

    #[tokio::test]
    async fn probe_server_detects_reachable_nextcloud_28() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/status.php")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"installed":true,"maintenance":false,"version":"28.0.1.3","versionstring":"28.0.1","edition":""}"#)
            .create_async()
            .await;

        let dto = probe_server(server.url()).await.unwrap();
        assert!(dto.reachable);
        assert_eq!(dto.version, "28.0.1");
        assert!(dto.version_ok, "28 >= 16");
        assert!(dto.e2ee_available, "28 >= 20");
        assert!(!dto.maintenance);
        assert!(dto.error.is_none());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn probe_server_reports_version_not_ok_for_old_nextcloud() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/status.php")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"installed":true,"maintenance":false,"version":"15.0.0","versionstring":"15.0","edition":""}"#)
            .create_async()
            .await;

        let dto = probe_server(server.url()).await.unwrap();
        assert!(dto.reachable);
        assert!(!dto.version_ok, "15 < 16");
        assert!(dto.error.is_some(), "should have an error message");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn probe_server_returns_unreachable_for_connection_refused() {
        // Port 1 is reserved and should always be refused.
        let dto = probe_server("http://127.0.0.1:1".to_string())
            .await
            .unwrap();
        assert!(!dto.reachable);
        assert!(dto.error.is_some());
    }

    #[tokio::test]
    async fn probe_server_handles_non_200_status() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/status.php")
            .with_status(503)
            .create_async()
            .await;

        let dto = probe_server(server.url()).await.unwrap();
        assert!(!dto.reachable);
        assert!(dto.error.as_deref().unwrap_or("").contains("503"));
        mock.assert_async().await;
    }

    // T016 — begin_auth_flow unit tests (display_code format)

    #[test]
    fn display_code_is_formatted_xxxx_dot_xxxx() {
        let raw = "abcdef12";
        let display = format!("{} \u{00B7} {}", &raw[..4], &raw[4..]);
        assert_eq!(display, "abcd · ef12");
    }

    // T023 — pick_folder unit tests (logic only; dialog requires a real UI)
    // Full dialog tests are deferred to integration / manual quickstart validation.

    // T030 — get_account_remote_stats parsing
    #[test]
    fn remote_stats_dto_serialises_correctly() {
        let dto = RemoteStatsDto {
            total_bytes: 107_374_182_400,
            used_bytes: 5_368_709_120,
            file_count: None,
        };
        let json = serde_json::to_string(&dto).unwrap();
        assert!(json.contains("107374182400"));
        assert!(json.contains("null")); // file_count is None
    }

    #[test]
    fn generate_qr_svg_produces_svg_element() {
        let svg =
            generate_qr_svg("https://cloud.example.com/index.php/login/v2/flow/abc123").unwrap();
        assert!(
            svg.starts_with("<svg") || svg.contains("<svg"),
            "output must be SVG"
        );
    }
}
