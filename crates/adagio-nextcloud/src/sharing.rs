use adagio_core::error::SyncError;
use reqwest::{header, Client, StatusCode};
use serde::Deserialize;
use tracing::instrument;

fn pct_encode(s: &str) -> String {
    s.bytes()
        .flat_map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![b as char]
            }
            b => vec![
                '%',
                char::from_digit((b >> 4) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
                char::from_digit((b & 0xf) as u32, 16)
                    .unwrap()
                    .to_ascii_uppercase(),
            ],
        })
        .collect()
}

// ── Public types ──────────────────────────────────────────────────────────────

/// A single user/group result from the Nextcloud sharees endpoint.
pub struct UserSearchResult {
    pub user_id: String,
    pub display_name: String,
}

/// A single recipient in a share request.
#[derive(Debug)]
pub struct ShareRecipient {
    pub user_id: String,
    /// Nextcloud permission bitmask: 1=view, 17=comment, 31=edit.
    pub permission: i32,
}

/// Input to [`create_share`].
#[derive(Debug)]
pub struct ShareRequest {
    pub path: String,
    pub recipients: Vec<ShareRecipient>,
    pub expiry_date: Option<String>,
    pub link_password: Option<String>,
    pub hide_download: bool,
    pub notify_on_open: bool,
    pub note: Option<String>,
}

/// Result from [`create_share`].
pub struct ShareResultDto {
    pub share_id: String,
    pub share_url: String,
}

// ── Internal OCS response shapes ─────────────────────────────────────────────

#[derive(Deserialize)]
struct OcsEnvelope<T> {
    ocs: OcsBody<T>,
}

#[derive(Deserialize)]
struct OcsBody<T> {
    data: T,
}

#[derive(Deserialize)]
struct ShareesData {
    users: Option<Vec<ShareeUser>>,
}

#[derive(Deserialize)]
struct ShareeUser {
    label: String,
    value: ShareeValue,
}

#[derive(Deserialize)]
struct ShareeValue {
    #[serde(rename = "shareWith")]
    share_with: String,
}

#[derive(Deserialize)]
struct ShareData {
    id: u64,
    url: Option<String>,
}

// ── API functions ─────────────────────────────────────────────────────────────

fn build_client() -> Result<Client, SyncError> {
    Client::builder()
        .user_agent("adagio-nextcloud/0.1")
        .build()
        .map_err(|e| SyncError::Permanent(format!("http client build error: {e}")))
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

/// Search the Nextcloud sharees endpoint for users matching `query`.
///
/// Returns an empty `Vec` immediately when `query` is empty (no network call).
#[instrument(skip(access_token))]
pub async fn search_sharees(
    server_url: &str,
    access_token: &str,
    query: &str,
) -> Result<Vec<UserSearchResult>, SyncError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let client = build_client()?;
    let url = format!(
        "{}/ocs/v2.php/apps/files_sharing/api/v1/sharees?format=json&search={}&itemType=file",
        server_url.trim_end_matches('/'),
        pct_encode(query),
    );

    let resp = client
        .get(&url)
        .header(header::AUTHORIZATION, bearer(access_token))
        .header("OCS-APIREQUEST", "true")
        .send()
        .await
        .map_err(|e| {
            SyncError::Transfer(adagio_core::error::TransferError::Transient(format!(
                "network error: {e}"
            )))
        })?;

    if resp.status() == StatusCode::UNAUTHORIZED || resp.status() == StatusCode::FORBIDDEN {
        return Err(SyncError::Permanent("Authentication failed".into()));
    }
    if !resp.status().is_success() {
        return Err(SyncError::Transfer(
            adagio_core::error::TransferError::Transient(format!(
                "sharees request failed: HTTP {}",
                resp.status()
            )),
        ));
    }

    let envelope: OcsEnvelope<ShareesData> = resp
        .json()
        .await
        .map_err(|e| SyncError::Permanent(format!("sharees response parse error: {e}")))?;

    let results = envelope
        .ocs
        .data
        .users
        .unwrap_or_default()
        .into_iter()
        .map(|u| UserSearchResult {
            user_id: u.value.share_with,
            display_name: u.label,
        })
        .collect();

    Ok(results)
}

/// Create one or more Nextcloud shares for `path`.
///
/// Creates a user-share per recipient, then a public link share.
/// Returns the public link URL and the ID of the first created share.
#[instrument(skip(access_token))]
pub async fn create_share(
    server_url: &str,
    access_token: &str,
    req: &ShareRequest,
) -> Result<ShareResultDto, SyncError> {
    let client = build_client()?;
    let base = format!(
        "{}/ocs/v2.php/apps/files_sharing/api/v1/shares",
        server_url.trim_end_matches('/')
    );

    let mut first_id: Option<u64> = None;
    let share_url;

    // Create per-recipient user shares
    for r in &req.recipients {
        let mut params = format!(
            "path={}&shareType=0&shareWith={}&permissions={}",
            pct_encode(&req.path),
            pct_encode(&r.user_id),
            r.permission,
        );
        if let Some(exp) = &req.expiry_date {
            params.push_str(&format!("&expireDate={}", pct_encode(exp)));
        }
        if let Some(note) = &req.note {
            params.push_str(&format!("&note={}", pct_encode(note)));
        }

        let resp = client
            .post(&base)
            .header(header::AUTHORIZATION, bearer(access_token))
            .header("OCS-APIREQUEST", "true")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(params)
            .send()
            .await
            .map_err(|e| {
                SyncError::Transfer(adagio_core::error::TransferError::Transient(format!(
                    "network error: {e}"
                )))
            })?;

        check_share_status(resp.status(), &base)?;

        let data: OcsEnvelope<ShareData> = resp
            .json()
            .await
            .map_err(|e| SyncError::Permanent(format!("share response parse error: {e}")))?;

        if first_id.is_none() {
            first_id = Some(data.ocs.data.id);
        }
    }

    // Create public link share
    {
        let mut params = format!("path={}&shareType=3&permissions=1", pct_encode(&req.path),);
        if let Some(pw) = &req.link_password {
            params.push_str(&format!("&password={}", pct_encode(pw)));
        }
        if req.hide_download {
            params.push_str("&hideDownload=true");
        }
        if let Some(exp) = &req.expiry_date {
            params.push_str(&format!("&expireDate={}", pct_encode(exp)));
        }

        let resp = client
            .post(&base)
            .header(header::AUTHORIZATION, bearer(access_token))
            .header("OCS-APIREQUEST", "true")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(params)
            .send()
            .await
            .map_err(|e| {
                SyncError::Transfer(adagio_core::error::TransferError::Transient(format!(
                    "network error: {e}"
                )))
            })?;

        check_share_status(resp.status(), &base)?;

        let data: OcsEnvelope<ShareData> = resp
            .json()
            .await
            .map_err(|e| SyncError::Permanent(format!("share response parse error: {e}")))?;

        share_url = data.ocs.data.url.unwrap_or_default();
        if first_id.is_none() {
            first_id = Some(data.ocs.data.id);
        }
    }

    Ok(ShareResultDto {
        share_id: first_id.unwrap_or(0).to_string(),
        share_url,
    })
}

fn check_share_status(status: StatusCode, url: &str) -> Result<(), SyncError> {
    match status.as_u16() {
        200 | 201 => Ok(()),
        401 | 403 => Err(SyncError::Permanent(
            "You don't have permission to share this item".into(),
        )),
        404 => Err(SyncError::Permanent("File not found on server".into())),
        s if s >= 500 => Err(SyncError::Transfer(
            adagio_core::error::TransferError::Transient(format!("server error {s}: {url}")),
        )),
        s => Err(SyncError::Permanent(format!(
            "unexpected status {s}: {url}"
        ))),
    }
}
