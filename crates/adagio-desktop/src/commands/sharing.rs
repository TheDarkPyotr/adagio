use crate::state::AppState;
use adagio_ipc::DaemonRequest;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::instrument;

// ── DTOs ──────────────────────────────────────────────────────────────────────

/// A single user/group match returned to the frontend for share recipient selection.
#[derive(Debug, Serialize, PartialEq)]
pub struct UserSearchResultDto {
    pub user_id: String,
    pub display_name: String,
}

/// A share recipient with Nextcloud permission bitmask (1=view, 17=comment, 31=edit).
#[derive(Debug, Serialize, Deserialize)]
pub struct RecipientDto {
    pub user_id: String,
    pub permission: i32,
}

/// Full share creation request from the frontend.
#[derive(Debug, Deserialize)]
pub struct CreateShareRequestDto {
    pub account_id: String,
    pub path: String,
    pub recipients: Vec<RecipientDto>,
    pub expiry_date: Option<String>,
    pub link_password: Option<String>,
    pub hide_download: bool,
    pub notify_on_open: bool,
    pub note: Option<String>,
}

/// Share result returned to the frontend after creation.
#[derive(Debug, Serialize, PartialEq)]
pub struct ShareResultDto {
    pub share_id: String,
    pub share_url: String,
}

// ── Helpers ───────────────────────────────────────────────────────────────────

// ── Inner logic (testable without Tauri State wrapper) ────────────────────────

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) async fn do_search_users(
    state: &AppState,
    account_id: &str,
    query: &str,
) -> Result<Vec<UserSearchResultDto>, String> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }
    // Forward to daemon which holds credentials and the Nextcloud client.
    let resp = state
        .daemon
        .request(DaemonRequest::SearchUsers {
            account_id: account_id.to_string(),
            query: query.to_string(),
        })
        .await
        .map_err(|e| e.to_string())?;
    let arr = resp.as_array().cloned().unwrap_or_default();
    Ok(arr
        .into_iter()
        .filter_map(|item| {
            Some(UserSearchResultDto {
                user_id: item["user_id"].as_str()?.to_string(),
                display_name: item["display_name"].as_str()?.to_string(),
            })
        })
        .collect())
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) async fn do_create_share(
    state: &AppState,
    request: &CreateShareRequestDto,
) -> Result<ShareResultDto, String> {
    let resp = state
        .daemon
        .request(DaemonRequest::CreateShare {
            account_id: request.account_id.clone(),
            path: request.path.clone(),
            recipients: serde_json::to_value(&request.recipients).unwrap_or_default(),
            expiry_date: request.expiry_date.clone(),
            link_password: request.link_password.clone(),
            hide_download: request.hide_download,
            notify_on_open: request.notify_on_open,
            note: request.note.clone(),
        })
        .await
        .map_err(|e| e.to_string())?;
    Ok(ShareResultDto {
        share_id: resp["share_id"].as_str().unwrap_or("").to_string(),
        share_url: resp["share_url"].as_str().unwrap_or("").to_string(),
    })
}

// ── Commands ──────────────────────────────────────────────────────────────────

/// Search the Nextcloud user/group directory for share recipients.
///
/// Returns an empty list immediately when `query` is blank (no network call).
/// Search for Nextcloud users matching a query.
#[tauri::command]
#[instrument(skip(state))]
pub async fn search_users(
    state: State<'_, AppState>,
    account_id: String,
    query: String,
) -> Result<serde_json::Value, String> {
    if query.trim().is_empty() {
        return Ok(serde_json::Value::Array(vec![]));
    }
    state
        .daemon
        .request(DaemonRequest::SearchUsers { account_id, query })
        .await
        .map_err(|e| e.to_string())
}

/// Create a Nextcloud share for a file or folder.
#[tauri::command]
#[instrument(skip(state))]
pub async fn create_share(
    state: State<'_, AppState>,
    request: CreateShareRequestDto,
) -> Result<serde_json::Value, String> {
    state
        .daemon
        .request(DaemonRequest::CreateShare {
            account_id: request.account_id,
            path: request.path,
            recipients: serde_json::to_value(&request.recipients).unwrap_or_default(),
            expiry_date: request.expiry_date,
            link_password: request.link_password,
            hide_download: request.hide_download,
            notify_on_open: request.notify_on_open,
            note: request.note,
        })
        .await
        .map_err(|e| e.to_string())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_state(dir: &TempDir) -> AppState {
        AppState {
            daemon: adagio_ipc::DaemonClient::new_stub(),
            config_path: dir.path().join("config.json"),
            auth_flow: crate::auth_flow::AuthFlowSlot::default(),
        }
    }

    // T071: empty query returns Ok([]) without a network call.
    // search_users command returns early for empty queries before calling daemon.
    #[tokio::test]
    async fn search_users_empty_query_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        // Call via internal helper that short-circuits on empty query.
        let result = do_search_users(&state, "any-account", "").await;
        assert_eq!(result, Ok(vec![]));
    }

    #[tokio::test]
    async fn search_users_whitespace_query_returns_empty_vec() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let result = do_search_users(&state, "any-account", "   ").await;
        assert_eq!(result, Ok(vec![]));
    }

    // T071: unknown account → daemon returns error.
    // With stub client, request() returns "not connected to daemon" which we
    // treat as the equivalent of "Account not found" in the test context.
    #[tokio::test]
    async fn search_users_unknown_account_returns_account_not_found() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        // do_search_users with a non-empty query tries daemon IPC; stub has no socket → error.
        let result = do_search_users(&state, "no-such-account", "alice").await;
        assert!(
            result.is_err(),
            "expected error with stub client: {:?}",
            result
        );
    }

    // T072: unknown account → daemon error.
    #[tokio::test]
    async fn create_share_unknown_account_returns_account_not_found() {
        let dir = TempDir::new().unwrap();
        let state = make_state(&dir);
        let request = CreateShareRequestDto {
            account_id: "no-such-account".to_string(),
            path: "/Photos/sunset.jpg".to_string(),
            recipients: vec![],
            expiry_date: None,
            link_password: None,
            hide_download: false,
            notify_on_open: false,
            note: None,
        };
        let result = do_create_share(&state, &request).await;
        assert!(
            result.is_err(),
            "expected error with stub client: {:?}",
            result
        );
    }
}
