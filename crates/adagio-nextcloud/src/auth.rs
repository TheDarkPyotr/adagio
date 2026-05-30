use adagio_core::error::SyncError;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tracing::{debug, error, info, instrument};

/// Bundled OAuth2 client credentials for the Nextcloud OAuth2 app.
///
/// Production values are injected at release time via CI environment variables
/// (`ADAGIO_OAUTH2_CLIENT_ID` / `ADAGIO_OAUTH2_CLIENT_SECRET`). The defaults
/// below are for local development only and must be registered in the target
/// Nextcloud instance's admin panel at `/settings/admin/security`.
pub const OAUTH2_CLIENT_ID: &str =
    "3UZ8GxFECiQB3yLgQx1Znv6eTLDHUJrrK6AGtkPWwkNnM7zkPVH65SczbswQzzFP";
pub const OAUTH2_CLIENT_SECRET: &str =
    "8w0TRTZm7DejDsNM9gtWJzXsN95YTRqMbydSandKd4exNj9njq5WYQjCMaKb8yYR";

/// Generate a PKCE (code_verifier, code_challenge) pair using S256 method.
///
/// `code_verifier`: 32 cryptographically-random bytes, base64url-encoded (no padding).
/// `code_challenge`: SHA-256(verifier_bytes) base64url-encoded (no padding).
pub fn generate_pkce_pair() -> (String, String) {
    let random_bytes = random_bytes_32();
    let verifier = URL_SAFE_NO_PAD.encode(random_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    (verifier, challenge)
}

/// Generate a random opaque state parameter for OAuth2 CSRF protection.
pub fn generate_state() -> String {
    URL_SAFE_NO_PAD.encode(random_bytes_32())
}

/// Build the OAuth2 authorization URL for a Nextcloud server.
pub fn build_authorization_url(
    server_url: &str,
    client_id: &str,
    redirect_uri: &str,
    code_challenge: &str,
    state: &str,
) -> String {
    let base = server_url.trim_end_matches('/');
    // redirect_uri must be percent-encoded when embedded in the query string.
    let encoded_redirect = percent_encode(redirect_uri);
    format!(
        "{base}/index.php/apps/oauth2/authorize\
         ?response_type=code\
         &client_id={client_id}\
         &redirect_uri={encoded_redirect}\
         &code_challenge={code_challenge}\
         &code_challenge_method=S256\
         &state={state}"
    )
}

/// Probe the server URL to confirm it is a reachable Nextcloud instance.
///
/// Sends a GET to `{server_url}/status.php` with a 5-second timeout and
/// verifies the JSON response contains `"installed": true`.
///
/// # Errors
/// - `SyncError::Permanent("Server unreachable: …")` — network error or non-200 response.
/// - `SyncError::Permanent("Not a Nextcloud server: …")` — 200 OK but not a valid NC status.
#[instrument(skip_all, fields(server_url))]
pub async fn validate_server_url(server_url: &str) -> Result<(), SyncError> {
    let url = format!("{}/status.php", server_url.trim_end_matches('/'));
    debug!(url = %url, "probing Nextcloud status endpoint");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| SyncError::Permanent(format!("http client build error: {e}")))?;

    let resp = client.get(&url).send().await.map_err(|e| {
        error!(error = %e, "server unreachable");
        SyncError::Permanent(format!("Server unreachable: {e}"))
    })?;

    if !resp.status().is_success() {
        let status = resp.status();
        error!(status = %status, "status.php returned non-success");
        return Err(SyncError::Permanent(format!(
            "Server unreachable: status.php returned {status}"
        )));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        error!(error = %e, "status.php response not valid JSON");
        SyncError::Permanent(format!("Not a Nextcloud server: {e}"))
    })?;

    if json.get("installed").and_then(|v| v.as_bool()) != Some(true) {
        error!("status.php missing 'installed:true'");
        return Err(SyncError::Permanent(
            "Not a Nextcloud server: missing installed:true in status.php".to_string(),
        ));
    }

    info!(server_url, "Nextcloud server validated");
    Ok(())
}

/// Fetch the authenticated user's display name and username from the OCS API.
///
/// Returns `(username, display_name)` extracted from `/ocs/v2.php/cloud/user`.
///
/// # Errors
/// - `SyncError::Permanent` if the request fails, returns non-200, or the response
///   is missing required fields.
#[instrument(skip(access_token), fields(server_url))]
pub async fn fetch_user_info(
    server_url: &str,
    access_token: &str,
) -> Result<(String, String), SyncError> {
    let url = format!(
        "{}/ocs/v2.php/cloud/user?format=json",
        server_url.trim_end_matches('/')
    );
    debug!(url = %url, "fetching user info");

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| SyncError::Permanent(format!("http client build error: {e}")))?;

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {access_token}"))
        .header("OCS-APIREQUEST", "true")
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "user info request failed");
            SyncError::Permanent(format!("user info request failed: {e}"))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        error!(status = %status, "user info endpoint returned non-success");
        return Err(SyncError::Permanent(format!(
            "user info endpoint returned {status}"
        )));
    }

    let json: serde_json::Value = resp.json().await.map_err(|e| {
        error!(error = %e, "user info response not valid JSON");
        SyncError::Permanent(format!("bad user info response: {e}"))
    })?;

    let data = json
        .pointer("/ocs/data")
        .ok_or_else(|| SyncError::Permanent("user info response missing ocs.data".to_string()))?;

    let username = data["id"]
        .as_str()
        .ok_or_else(|| SyncError::Permanent("user info missing id field".to_string()))?
        .to_string();

    let display_name = data["display-name"]
        .as_str()
        .unwrap_or(&username)
        .to_string();

    info!(username = %username, "fetched user info");
    Ok((username, display_name))
}

/// Token pair stored after a successful OAuth2 flow or refresh.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TokenPair {
    pub access_token: String,
    pub refresh_token: String,
}

/// Exchange an authorization code for an access token + refresh token.
#[instrument(skip(client_secret, code, code_verifier), fields(server_url))]
pub async fn exchange_code(
    server_url: &str,
    client_id: &str,
    client_secret: &str,
    code: &str,
    code_verifier: &str,
    redirect_uri: &str,
) -> Result<TokenPair, SyncError> {
    let url = format!(
        "{}/index.php/apps/oauth2/api/v1/token",
        server_url.trim_end_matches('/')
    );
    debug!(url = %url, "exchanging authorization code for tokens");

    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("code_verifier", code_verifier),
            ("redirect_uri", redirect_uri),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "token exchange request failed");
            SyncError::Permanent(format!("token exchange failed: {e}"))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        error!(status = %status, "token exchange returned non-success");
        return Err(SyncError::Permanent(format!(
            "token endpoint returned {status}"
        )));
    }

    let json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Permanent(format!("bad token response: {e}")))?;

    let pair = TokenPair {
        access_token: json["access_token"].as_str().unwrap_or("").to_string(),
        refresh_token: json["refresh_token"].as_str().unwrap_or("").to_string(),
    };
    info!("authorization code exchanged successfully");
    Ok(pair)
}

/// Refresh an expired access token using the stored refresh token.
///
/// On success, stores the updated token pair in the keychain and returns the new access token.
#[instrument(skip(client_secret), fields(server_url, account_id))]
pub async fn refresh_access_token(
    server_url: &str,
    client_id: &str,
    client_secret: &str,
    account_id: &str,
) -> Result<String, SyncError> {
    debug!("refreshing access token");

    let stored_json = tokio::task::spawn_blocking({
        let account_id = account_id.to_string();
        move || retrieve_credentials(&account_id)
    })
    .await
    .map_err(|e| SyncError::Permanent(e.to_string()))??;

    let json_str = stored_json.ok_or_else(|| {
        SyncError::Permanent(format!("no credentials found for account {account_id}"))
    })?;

    let pair: TokenPair = serde_json::from_str(&json_str)
        .map_err(|e| SyncError::Permanent(format!("bad stored token: {e}")))?;

    let url = format!(
        "{}/index.php/apps/oauth2/api/v1/token",
        server_url.trim_end_matches('/')
    );
    let client = reqwest::Client::new();
    let resp = client
        .post(&url)
        .form(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", &pair.refresh_token),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .map_err(|e| {
            error!(error = %e, "token refresh request failed");
            SyncError::Permanent(format!("token refresh request failed: {e}"))
        })?;

    if !resp.status().is_success() {
        let status = resp.status();
        error!(status = %status, "token refresh returned non-success");
        return Err(SyncError::Permanent(format!(
            "token refresh returned {status}"
        )));
    }

    let new_json: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| SyncError::Permanent(format!("bad refresh response: {e}")))?;

    let new_access = new_json["access_token"].as_str().unwrap_or("").to_string();
    let new_refresh = new_json
        .get("refresh_token")
        .and_then(|v| v.as_str())
        .unwrap_or(&pair.refresh_token)
        .to_string();

    let new_pair = TokenPair {
        access_token: new_access.clone(),
        refresh_token: new_refresh,
    };
    let new_json_str = serde_json::to_string(&new_pair)
        .map_err(|e| SyncError::Permanent(format!("serialize token: {e}")))?;

    tokio::task::spawn_blocking({
        let account_id = account_id.to_string();
        move || store_credentials(&account_id, &new_json_str)
    })
    .await
    .map_err(|e| SyncError::Permanent(e.to_string()))??;

    info!("access token refreshed successfully");
    Ok(new_access)
}

/// Store credentials in the OS keychain under `("adagio", account_id)`.
///
/// MUST be called from a `spawn_blocking` context — keyring uses sync I/O.
pub fn store_credentials(account_id: &str, secret: &str) -> Result<(), SyncError> {
    let entry = keyring::Entry::new("adagio", account_id)
        .map_err(|e| SyncError::Permanent(format!("keyring entry error: {e}")))?;
    entry
        .set_password(secret)
        .map_err(|e| SyncError::Permanent(format!("keyring set error: {e}")))?;
    debug!(account_id, "credentials stored in keychain");
    Ok(())
}

/// Retrieve credentials from the OS keychain.
///
/// MUST be called from a `spawn_blocking` context — keyring uses sync I/O.
pub fn retrieve_credentials(account_id: &str) -> Result<Option<String>, SyncError> {
    let entry = keyring::Entry::new("adagio", account_id)
        .map_err(|e| SyncError::Permanent(format!("keyring entry error: {e}")))?;
    match entry.get_password() {
        Ok(pw) => {
            debug!(account_id, "credentials retrieved from keychain");
            Ok(Some(pw))
        }
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(SyncError::Permanent(format!("keyring get error: {e}"))),
    }
}

/// Delete credentials from the OS keychain.
///
/// MUST be called from a `spawn_blocking` context — keyring uses sync I/O.
pub fn delete_credentials(account_id: &str) -> Result<(), SyncError> {
    let entry = keyring::Entry::new("adagio", account_id)
        .map_err(|e| SyncError::Permanent(format!("keyring entry error: {e}")))?;
    match entry.delete_credential() {
        Ok(()) => {
            info!(account_id, "credentials deleted from keychain");
            Ok(())
        }
        Err(keyring::Error::NoEntry) => Ok(()),
        Err(e) => Err(SyncError::Permanent(format!("keyring delete error: {e}"))),
    }
}

/// Percent-encode a string for use as a query-parameter value.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            other => {
                out.push('%');
                out.push(
                    char::from_digit((other >> 4) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((other & 0xf) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}

fn random_bytes_32() -> [u8; 32] {
    // Two UUID v4 values (getrandom-backed) hashed together to produce 32 bytes.
    let a = uuid::Uuid::new_v4();
    let b = uuid::Uuid::new_v4();
    let mut seed = [0u8; 32];
    seed[..16].copy_from_slice(a.as_bytes());
    seed[16..].copy_from_slice(b.as_bytes());
    let digest = Sha256::digest(seed);
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncReadExt;
    use tokio::net::TcpListener;

    // ── PKCE tests ────────────────────────────────────────────────────────────

    #[test]
    fn pkce_verifier_is_not_empty() {
        let (verifier, _) = generate_pkce_pair();
        assert!(!verifier.is_empty(), "code_verifier must not be empty");
    }

    #[test]
    fn pkce_challenge_is_not_empty() {
        let (_, challenge) = generate_pkce_pair();
        assert!(!challenge.is_empty(), "code_challenge must not be empty");
    }

    #[test]
    fn pkce_challenge_is_base64url_no_padding() {
        let (_, challenge) = generate_pkce_pair();
        assert!(
            challenge
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'),
            "code_challenge must be base64url (no padding): got {challenge:?}"
        );
        assert!(
            !challenge.contains('='),
            "base64url-encoded challenge must have no padding characters"
        );
    }

    #[test]
    fn pkce_challenge_is_sha256_of_verifier() {
        let (verifier, challenge) = generate_pkce_pair();
        let expected = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
        assert_eq!(
            challenge, expected,
            "code_challenge must be SHA256(verifier) base64url-encoded (S256 method)"
        );
    }

    #[test]
    fn pkce_verifier_differs_between_calls() {
        let (v1, _) = generate_pkce_pair();
        let (v2, _) = generate_pkce_pair();
        assert_ne!(v1, v2, "each call must produce a unique verifier");
    }

    #[test]
    fn state_is_not_empty() {
        assert!(!generate_state().is_empty());
    }

    #[test]
    fn state_differs_between_calls() {
        assert_ne!(generate_state(), generate_state());
    }

    #[test]
    fn authorization_url_contains_challenge_and_state() {
        let (_, challenge) = generate_pkce_pair();
        let state = generate_state();
        let url = build_authorization_url(
            "https://cloud.example.com",
            "adagio",
            "http://127.0.0.1:9876/callback",
            &challenge,
            &state,
        );
        assert!(url.contains(&challenge), "URL must contain code_challenge");
        assert!(url.contains(&state), "URL must contain state");
        assert!(url.contains("code_challenge_method=S256"));
    }

    #[test]
    fn authorization_url_encodes_redirect_uri() {
        let (_, challenge) = generate_pkce_pair();
        let state = generate_state();
        let url = build_authorization_url(
            "https://cloud.example.com",
            "adagio",
            "http://127.0.0.1:9876/callback",
            &challenge,
            &state,
        );
        // The redirect_uri query param must be percent-encoded
        assert!(
            url.contains("redirect_uri=http%3A%2F%2F"),
            "redirect_uri must be percent-encoded in the URL"
        );
    }

    // ── T005: validate_server_url tests ───────────────────────────────────────

    async fn serve_one(listener: TcpListener, response: &'static str) {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 4096];
        let _ = stream.read(&mut buf).await;
        use tokio::io::AsyncWriteExt;
        stream.write_all(response.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn validate_server_url_ok_for_valid_nextcloud() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = r#"{"installed":true,"maintenance":false,"version":"28.0.1"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        tokio::spawn(serve_one(listener, response));
        let result = validate_server_url(&format!("http://127.0.0.1:{port}")).await;
        assert!(result.is_ok(), "valid NC server must return Ok: {result:?}");
    }

    #[tokio::test]
    async fn validate_server_url_err_for_non_200() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
        tokio::spawn(serve_one(listener, response));
        let result = validate_server_url(&format!("http://127.0.0.1:{port}")).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("unreachable") || msg.contains("404"),
            "got: {msg}"
        );
    }

    #[tokio::test]
    async fn validate_server_url_err_for_non_nextcloud_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = r#"{"name":"some-other-service"}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        tokio::spawn(serve_one(listener, response));
        let result = validate_server_url(&format!("http://127.0.0.1:{port}")).await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Not a Nextcloud server"), "got: {msg}");
    }

    #[tokio::test]
    async fn validate_server_url_err_for_unreachable_host() {
        // 192.0.2.x is TEST-NET-1 — guaranteed unroutable
        let result = validate_server_url("http://192.0.2.1").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("Server unreachable"), "got: {msg}");
    }

    // ── T006: fetch_user_info tests ───────────────────────────────────────────

    #[tokio::test]
    async fn fetch_user_info_extracts_username_and_display_name() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = r#"{"ocs":{"meta":{"status":"ok"},"data":{"id":"alice","display-name":"Alice Müller"}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        tokio::spawn(serve_one(listener, response));
        let result = fetch_user_info(&format!("http://127.0.0.1:{port}"), "test-token").await;
        let (username, display_name) = result.expect("should succeed");
        assert_eq!(username, "alice");
        assert_eq!(display_name, "Alice Müller");
    }

    #[tokio::test]
    async fn fetch_user_info_falls_back_to_username_when_display_name_missing() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = r#"{"ocs":{"data":{"id":"bob"}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        tokio::spawn(serve_one(listener, response));
        let (username, display_name) = fetch_user_info(&format!("http://127.0.0.1:{port}"), "tok")
            .await
            .unwrap();
        assert_eq!(username, "bob");
        assert_eq!(display_name, "bob", "should fall back to username");
    }

    #[tokio::test]
    async fn fetch_user_info_err_on_401() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let response = "HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\n\r\n";
        tokio::spawn(serve_one(listener, response));
        let result = fetch_user_info(&format!("http://127.0.0.1:{port}"), "bad-token").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("401"), "got: {msg}");
    }

    #[tokio::test]
    async fn fetch_user_info_err_when_id_missing() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let body = r#"{"ocs":{"data":{"display-name":"No ID here"}}}"#;
        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            body.len(),
            body
        );
        let response: &'static str = Box::leak(response.into_boxed_str());
        tokio::spawn(serve_one(listener, response));
        let result = fetch_user_info(&format!("http://127.0.0.1:{port}"), "tok").await;
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(msg.contains("missing id"), "got: {msg}");
    }

    // ── T022: SC-003 error-message timing ─────────────────────────────────────

    #[tokio::test]
    async fn validate_server_url_returns_error_within_6s_for_unroutable_host() {
        let start = std::time::Instant::now();
        let result = validate_server_url("http://192.0.2.1").await;
        let elapsed = start.elapsed();
        assert!(result.is_err(), "expected Err for unroutable host");
        assert!(
            elapsed.as_secs() < 6,
            "error must arrive within 6 s (5 s timeout + 1 s margin); took {elapsed:?}"
        );
    }
}
