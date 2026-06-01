/// Background polling task for Nextcloud Login Flow v2 authentication.
///
/// This module manages the in-progress auth session and drives the polling
/// loop that resolves credentials and emits Tauri events to the frontend.
use adagio_nextcloud::login_flow::LoginCredentials;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, instrument};

// ── AuthFlowState ─────────────────────────────────────────────────────────────

/// Ephemeral state for an in-progress Login Flow v2 authentication session.
///
/// Held in `AppState.auth_flow` behind a `Mutex`. Created when `begin_auth_flow`
/// is called; dropped (and the task aborted) when auth completes or expires.
pub struct AuthFlowState {
    /// One-time poll token issued by the Nextcloud server.
    pub poll_token: String,
    /// URL to POST the poll token to while waiting for user authentication.
    pub poll_endpoint: String,
    /// The Nextcloud server URL that initiated this session.
    pub server_url: String,
    /// Handle to the background polling task. Abort on session cancellation.
    pub task_handle: JoinHandle<()>,
    /// Absolute deadline — the task emits auth-flow-expired when this elapses.
    pub expires_at: Instant,
}

impl std::fmt::Debug for AuthFlowState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthFlowState")
            .field("server_url", &self.server_url)
            .field("poll_endpoint", &self.poll_endpoint)
            .field("expires_at", &self.expires_at)
            .finish_non_exhaustive()
    }
}

// ── Poll task ─────────────────────────────────────────────────────────────────

/// Spawn the background polling loop for a Login Flow v2 session.
///
/// The task:
/// - Polls `poll_endpoint` with `poll_token` every 2 seconds.
/// - On successful credentials: calls `on_complete` and exits.
/// - On deadline elapsed: calls `on_expired` and exits.
/// - On poll error: logs the error and continues polling (transient failures
///   are common when the user takes a moment to authenticate).
///
/// Callers must store the returned [`JoinHandle`] in [`AuthFlowState`] so
/// the task can be aborted if `begin_auth_flow` is called again before
/// the current session completes.
#[instrument(skip_all, fields(server_url))]
pub fn spawn_poll_task<F, G>(
    poll_endpoint: String,
    poll_token: String,
    server_url: String,
    deadline: Instant,
    on_complete: F,
    on_expired: G,
) -> JoinHandle<()>
where
    F: FnOnce(LoginCredentials) + Send + 'static,
    G: FnOnce() + Send + 'static,
{
    tokio::spawn(async move {
        info!(server_url = %server_url, "Login Flow v2 poll task started");
        let poll_interval = std::time::Duration::from_secs(2);

        loop {
            if Instant::now() >= deadline {
                info!(server_url = %server_url, "Login Flow v2 session expired");
                on_expired();
                return;
            }

            match adagio_nextcloud::login_flow::poll_login_flow(&poll_endpoint, &poll_token).await {
                Ok(Some(creds)) => {
                    info!(
                        server_url = %server_url,
                        login_name = %creds.login_name,
                        "Login Flow v2 authentication complete"
                    );
                    on_complete(creds);
                    return;
                }
                Ok(None) => {
                    debug!(server_url = %server_url, "Login Flow v2 poll: waiting for user");
                }
                Err(e) => {
                    error!(server_url = %server_url, error = %e, "Login Flow v2 poll error (will retry)");
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    })
}

// ── Convenience type alias ────────────────────────────────────────────────────

/// Thread-safe container for the current auth flow session.
pub type AuthFlowSlot = Arc<Mutex<Option<AuthFlowState>>>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Duration;

    // T017 — auth_flow polling task tests

    #[tokio::test]
    async fn poll_task_calls_on_expired_when_deadline_already_elapsed() {
        let expired = Arc::new(AtomicBool::new(false));
        let expired_clone = expired.clone();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();

        // Deadline already in the past.
        let deadline = Instant::now() - Duration::from_secs(1);

        let handle = spawn_poll_task(
            "http://localhost:1/poll".to_string(),
            "tok".to_string(),
            "https://example.com".to_string(),
            deadline,
            move |_| {
                completed_clone.store(true, Ordering::SeqCst);
            },
            move || {
                expired_clone.store(true, Ordering::SeqCst);
            },
        );

        handle.await.unwrap();
        assert!(
            expired.load(Ordering::SeqCst),
            "on_expired should have been called"
        );
        assert!(
            !completed.load(Ordering::SeqCst),
            "on_complete should NOT have been called"
        );
    }

    #[tokio::test]
    async fn poll_task_calls_on_complete_when_credentials_arrive() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("POST", "/poll")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"{"server":"https://nc.test","loginName":"carol","appPassword":"pw99"}"#)
            .create_async()
            .await;

        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = completed.clone();
        let received_user = Arc::new(Mutex::new(String::new()));
        let received_user_clone = received_user.clone();

        let deadline = Instant::now() + Duration::from_secs(30);
        let poll_url = format!("{}/poll", server.url());

        let handle = spawn_poll_task(
            poll_url,
            "tok".to_string(),
            "https://nc.test".to_string(),
            deadline,
            move |creds| {
                *received_user_clone.lock().unwrap() = creds.login_name.clone();
                completed_clone.store(true, Ordering::SeqCst);
            },
            || {},
        );

        handle.await.unwrap();
        assert!(completed.load(Ordering::SeqCst));
        assert_eq!(*received_user.lock().unwrap(), "carol");
        mock.assert_async().await;
    }
}
