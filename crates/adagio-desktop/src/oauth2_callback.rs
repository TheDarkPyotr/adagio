/// Loopback HTTP listener for OAuth2 authorization callbacks (RFC 8252 §7.3).
///
/// The listener binds on `127.0.0.1:0`, obtains an ephemeral port, and waits
/// for a single incoming GET request of the form:
/// `GET /callback?code=<code>&state=<state> HTTP/1.1`
///
/// It validates the `state` parameter to prevent CSRF, sends a minimal HTML
/// success response to the browser, and resolves with the authorization code.
use adagio_core::error::SyncError;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tracing::{debug, error, info, instrument};

const CALLBACK_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

const SUCCESS_HTML: &str = "\
HTTP/1.1 200 OK\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Length: 120\r\n\
Connection: close\r\n\
\r\n\
<html><body style=\"font-family:sans-serif;text-align:center;padding:60px\">\
<h2>Authorization complete</h2><p>You can close this window.</p></body></html>";

const ERROR_HTML: &str = "\
HTTP/1.1 400 Bad Request\r\n\
Content-Type: text/html; charset=utf-8\r\n\
Content-Length: 80\r\n\
Connection: close\r\n\
\r\n\
<html><body><h2>Authorization failed</h2><p>Please return to Adagio.</p></body></html>";

/// Bind a loopback listener and return `(port, future)`.
///
/// The returned future resolves with the authorization code once the OAuth2
/// server redirects the browser to the callback URL. Call this before opening
/// the browser so the port is available before the redirect arrives.
///
/// The future applies a 5-minute timeout. It returns:
/// - `Ok(code)` — authorization code ready for token exchange.
/// - `Err(SyncError::Permanent("Authorization cancelled"))` — state mismatch.
/// - `Err(SyncError::Permanent("Authorization timed out"))` — timeout elapsed.
/// - `Err(SyncError::Permanent("…"))` — other I/O error.
#[instrument(skip(expected_state))]
pub async fn spawn_callback_listener(
    expected_state: &str,
) -> Result<
    (
        u16,
        impl std::future::Future<Output = Result<String, SyncError>>,
    ),
    SyncError,
> {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|e| SyncError::Permanent(format!("callback listener bind failed: {e}")))?;

    let port = listener
        .local_addr()
        .map_err(|e| SyncError::Permanent(format!("could not get listener port: {e}")))?
        .port();

    debug!(port, "callback listener bound");

    let expected_state = expected_state.to_string();
    let callback_fut = async move {
        let result =
            tokio::time::timeout(CALLBACK_TIMEOUT, accept_one(&listener, &expected_state)).await;
        match result {
            Ok(inner) => inner,
            Err(_) => {
                error!("OAuth2 callback timed out after 5 minutes");
                Err(SyncError::Permanent("Authorization timed out".to_string()))
            }
        }
    };

    Ok((port, callback_fut))
}

async fn accept_one(listener: &TcpListener, expected_state: &str) -> Result<String, SyncError> {
    let (mut stream, peer) = listener
        .accept()
        .await
        .map_err(|e| SyncError::Permanent(format!("callback accept failed: {e}")))?;

    debug!(peer = %peer, "browser connected to callback listener");

    // Read the HTTP request (first 4 KiB is more than enough for the request line).
    let mut buf = vec![0u8; 4096];
    let n = stream
        .read(&mut buf)
        .await
        .map_err(|e| SyncError::Permanent(format!("callback read failed: {e}")))?;

    let request = String::from_utf8_lossy(&buf[..n]);
    let first_line = request.lines().next().unwrap_or("");
    debug!(first_line = %first_line, "callback request received");

    // Extract query string from "GET /callback?code=...&state=... HTTP/1.1"
    let query = first_line
        .split_whitespace()
        .nth(1)
        .and_then(|path| path.split_once('?'))
        .map(|(_, qs)| qs)
        .unwrap_or("");

    let params = parse_query(query);
    let code = params.get("code").cloned().unwrap_or_default();
    let state = params.get("state").cloned().unwrap_or_default();

    if state != expected_state {
        error!("OAuth2 state mismatch — possible CSRF");
        let _ = stream.write_all(ERROR_HTML.as_bytes()).await;
        return Err(SyncError::Permanent(
            "Invalid authorization response".to_string(),
        ));
    }

    if code.is_empty() {
        let _ = stream.write_all(ERROR_HTML.as_bytes()).await;
        return Err(SyncError::Permanent("Authorization cancelled".to_string()));
    }

    let _ = stream.write_all(SUCCESS_HTML.as_bytes()).await;
    info!("authorization code received from browser");
    Ok(code)
}

/// Parse `key=value&key=value` query strings. Values are NOT percent-decoded
/// (Nextcloud sends plain ASCII codes and state values).
fn parse_query(query: &str) -> std::collections::HashMap<String, String> {
    query
        .split('&')
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let k = parts.next()?.to_string();
            let v = parts.next().unwrap_or("").to_string();
            if k.is_empty() {
                None
            } else {
                Some((k, v))
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    // ── T007: loopback callback listener tests ────────────────────────────────

    #[tokio::test]
    async fn listener_returns_nonzero_port() {
        let (port, _fut) = spawn_callback_listener("state123").await.unwrap();
        assert!(port > 0, "listener must bind to a real port");
    }

    #[tokio::test]
    async fn listener_resolves_with_code_on_valid_callback() {
        let (port, fut) = spawn_callback_listener("csrf-token-xyz").await.unwrap();
        let fut_handle = tokio::spawn(fut);

        // Simulate the browser hitting the callback URL
        let mut conn = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        conn.write_all(
            b"GET /callback?code=auth-code-123&state=csrf-token-xyz HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .await
        .unwrap();
        drop(conn);

        let code = fut_handle.await.unwrap().unwrap();
        assert_eq!(code, "auth-code-123");
    }

    #[tokio::test]
    async fn listener_rejects_state_mismatch() {
        let (port, fut) = spawn_callback_listener("expected-state").await.unwrap();
        let fut_handle = tokio::spawn(fut);

        let mut conn = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        conn.write_all(
            b"GET /callback?code=abc&state=wrong-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
        )
        .await
        .unwrap();
        drop(conn);

        let err = fut_handle.await.unwrap().unwrap_err();
        assert!(
            err.to_string().contains("Invalid authorization response"),
            "got: {err}"
        );
    }

    #[tokio::test]
    async fn listener_err_when_code_missing() {
        let (port, fut) = spawn_callback_listener("my-state").await.unwrap();
        let fut_handle = tokio::spawn(fut);

        let mut conn = TcpStream::connect(format!("127.0.0.1:{port}"))
            .await
            .unwrap();
        conn.write_all(b"GET /callback?state=my-state HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n")
            .await
            .unwrap();
        drop(conn);

        let err = fut_handle.await.unwrap().unwrap_err();
        assert!(
            err.to_string().contains("Authorization cancelled"),
            "got: {err}"
        );
    }

    #[test]
    fn parse_query_handles_basic_pairs() {
        let params = parse_query("code=abc123&state=xyz");
        assert_eq!(params.get("code").unwrap(), "abc123");
        assert_eq!(params.get("state").unwrap(), "xyz");
    }

    #[test]
    fn parse_query_handles_empty_string() {
        let params = parse_query("");
        assert!(params.is_empty());
    }
}
