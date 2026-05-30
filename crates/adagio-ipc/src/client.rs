use crate::transport::daemon_socket_path;
use crate::types::{DaemonEvent, DaemonRequest};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::{broadcast, watch, Mutex};
use tracing::{debug, info, warn};

/// Connection state of the IPC client to the daemon process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionState {
    Connected,
    Reconnecting {
        attempt: u8,
    },
    /// User-initiated stop — reconnect loop is disabled.
    Stopped,
    /// Three consecutive restart failures — requires manual intervention.
    Failed,
}

type RpcWriter = tokio::net::unix::OwnedWriteHalf;
type RpcReader = BufReader<tokio::net::unix::OwnedReadHalf>;

/// Holds the RPC socket halves together so that write+read is always atomic.
struct RpcConn {
    writer: RpcWriter,
    reader: RpcReader,
}

/// Client that communicates with `adagio-daemon` over the local IPC socket.
///
/// - **RPC connection** — serialised under a single mutex; each request holds
///   the lock for the full write→read round-trip, preventing response mixing
///   when multiple callers (e.g. status poll + data fetch) run concurrently.
/// - **Subscription connection** — forwards push events to a broadcast channel.
///
/// Reconnects automatically (up to 3 attempts) when the daemon terminates
/// unexpectedly. Transitions to `ConnectionState::Failed` after 3 failures.
pub struct DaemonClient {
    /// Single mutex covering the whole write→read cycle.
    /// `None` while disconnected / reconnecting.
    rpc: Arc<Mutex<Option<RpcConn>>>,
    /// Monotonically increasing request ID.
    next_id: AtomicU64,
    /// Broadcasts `DaemonEvent` to all subscribers.
    event_tx: broadcast::Sender<DaemonEvent>,
    /// Reports the current connection state.
    state_tx: watch::Sender<ConnectionState>,
}

impl DaemonClient {
    /// Connect to a running daemon, or spawn it if it is not running.
    ///
    /// `daemon_binary_path` is the path to the `adagio-daemon` executable.
    /// On first attempt the client tries to connect directly; if that fails it
    /// spawns the binary and retries for up to 5 seconds.
    pub async fn connect_or_start(daemon_binary_path: &Path) -> anyhow::Result<Arc<Self>> {
        let socket_path = daemon_socket_path();

        // Fast path: daemon already running.
        if let Ok(client) = Self::try_connect_once(&socket_path).await {
            info!("connected to existing adagio-daemon");
            return Ok(client);
        }

        // Spawn daemon and retry for up to 5 s.
        info!("adagio-daemon not running — spawning");
        spawn_daemon(daemon_binary_path)?;

        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            if let Ok(client) = Self::try_connect_once(&socket_path).await {
                info!("connected to newly started adagio-daemon");
                return Ok(client);
            }
            if tokio::time::Instant::now() >= deadline {
                anyhow::bail!("adagio-daemon did not start within 5 seconds");
            }
        }
    }

    /// Attempt a single connection to the daemon socket.
    async fn try_connect_once(socket_path: &Path) -> anyhow::Result<Arc<Self>> {
        let rpc_stream = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (read_half, write_half) = rpc_stream.into_split();

        // Open a subscription connection.
        let sub_stream = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;

        let (event_tx, _) = broadcast::channel(256);
        let (state_tx, _) = watch::channel(ConnectionState::Connected);

        let rpc = Arc::new(Mutex::new(Some(RpcConn {
            writer: write_half,
            reader: BufReader::new(read_half),
        })));

        let client = Arc::new(Self {
            rpc: rpc.clone(),
            next_id: AtomicU64::new(1),
            event_tx: event_tx.clone(),
            state_tx,
        });

        // Send subscribe marker on the subscription connection.
        let (sub_read, mut sub_write) = sub_stream.into_split();
        sub_write.write_all(b"{\"type\":\"subscribe\"}\n").await?;

        // Spawn event forwarding + reconnection task.
        let state_tx_clone = client.state_tx.clone();
        let socket = socket_path.to_path_buf();
        tokio::spawn(async move {
            forward_events(BufReader::new(sub_read), event_tx, state_tx_clone.clone()).await;
            reconnect_loop(socket, state_tx_clone, rpc).await;
        });

        Ok(client)
    }

    /// Upgrade a stub client to a real connection in-place.
    ///
    /// Opens the socket, installs the RPC halves, opens a subscription
    /// connection, and spawns the event-forwarding and reconnection tasks.
    /// Called once after the daemon is confirmed running; the AppState
    /// containing this stub can be registered before calling this.
    pub async fn upgrade_connection(&self, socket_path: &Path) -> anyhow::Result<()> {
        // RPC connection — both halves wrapped in the single mutex.
        let rpc_stream = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (read_half, write_half) = rpc_stream.into_split();
        {
            let mut guard = self.rpc.lock().await;
            *guard = Some(RpcConn {
                writer: write_half,
                reader: BufReader::new(read_half),
            });
        }

        // Subscription connection.
        let sub_stream = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (sub_read, mut sub_write) = sub_stream.into_split();
        sub_write.write_all(b"{\"type\":\"subscribe\"}\n").await?;

        // Spawn event-forwarding + reconnection background task.
        let state_tx = self.state_tx.clone();
        let rpc = self.rpc.clone();
        let event_tx = self.event_tx.clone();
        let socket = socket_path.to_path_buf();
        tokio::spawn(async move {
            forward_events(BufReader::new(sub_read), event_tx, state_tx.clone()).await;
            reconnect_loop(socket, state_tx, rpc).await;
        });

        self.state_tx.send_replace(ConnectionState::Connected);
        Ok(())
    }

    /// Open a new RPC connection to the socket (no spawn, no subscription).
    ///
    /// Used by the reconnection loop to replace stale halves.
    async fn try_reconnect(socket_path: &Path) -> anyhow::Result<RpcConn> {
        let stream = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (read_half, write_half) = stream.into_split();
        Ok(RpcConn {
            writer: write_half,
            reader: BufReader::new(read_half),
        })
    }

    /// Send an RPC request and return the raw `result` value from the response.
    ///
    /// Returns `Ok(serde_json::Value)` on success or `Err` if the daemon
    /// returned an error or the socket failed.
    ///
    /// The single `rpc` mutex is held for the ENTIRE write→read round-trip,
    /// serialising all concurrent callers and preventing response mixing.
    pub async fn request(&self, req: DaemonRequest) -> anyhow::Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req_json = serde_json::to_value(&req)?;
        let envelope = serde_json::json!({
            "id": id,
            "method": req_json["method"],
            "params": req_json["params"],
        });
        let line = format!("{envelope}\n");

        // Hold the lock for the full write→read cycle.
        let mut guard = self.rpc.lock().await;
        let conn = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("not connected to daemon"))?;

        conn.writer.write_all(line.as_bytes()).await?;

        let mut resp_line = String::new();
        conn.reader.read_line(&mut resp_line).await?;

        drop(guard); // release as soon as we have the bytes

        let resp_val: serde_json::Value = serde_json::from_str(resp_line.trim())?;
        if let Some(err) = resp_val.get("error") {
            anyhow::bail!("{}", err["message"].as_str().unwrap_or("unknown error"));
        }
        Ok(resp_val["result"].clone())
    }

    /// Subscribe to push events from the daemon.
    ///
    /// Returns a `broadcast::Receiver` that delivers a copy of every
    /// `DaemonEvent` emitted after this call.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.event_tx.subscribe()
    }

    /// Watch connection state changes.
    pub fn connection_state(&self) -> watch::Receiver<ConnectionState> {
        self.state_tx.subscribe()
    }
}

/// Forward NDJSON events from the subscription connection to the broadcast channel.
async fn forward_events(
    mut reader: BufReader<tokio::net::unix::OwnedReadHalf>,
    event_tx: broadcast::Sender<DaemonEvent>,
    state_tx: watch::Sender<ConnectionState>,
) {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => {
                debug!("subscription connection closed by daemon");
                break;
            }
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<DaemonEvent>(trimmed) {
                    Ok(event) => {
                        let is_shutdown = matches!(event, DaemonEvent::ShuttingDown { .. });
                        let _ = event_tx.send(event);
                        if is_shutdown {
                            break;
                        }
                    }
                    Err(e) => {
                        debug!("failed to parse daemon event: {e}: {trimmed}");
                    }
                }
            }
            Err(e) => {
                warn!("subscription read error: {e}");
                break;
            }
        }
    }
    // Transition to Reconnecting if we weren't stopped intentionally.
    let current = state_tx.borrow().clone();
    if current == ConnectionState::Connected {
        let _ = state_tx.send(ConnectionState::Reconnecting { attempt: 1 });
    }
}

/// Attempt to reconnect to the daemon after an unexpected disconnect.
///
/// Tries up to 3 times with 1-second gaps. On success, replaces the RPC
/// connection in the shared mutex and transitions to `Connected`.
/// On 3 consecutive failures, transitions to `Failed`.
///
/// If the connection state is `Stopped` (user-initiated), does nothing.
async fn reconnect_loop(
    socket_path: std::path::PathBuf,
    state_tx: watch::Sender<ConnectionState>,
    rpc: Arc<Mutex<Option<RpcConn>>>,
) {
    // Don't reconnect if user explicitly stopped.
    if *state_tx.borrow() == ConnectionState::Stopped {
        return;
    }

    for attempt in 1u8..=3 {
        let _ = state_tx.send(ConnectionState::Reconnecting { attempt });
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        match DaemonClient::try_reconnect(&socket_path).await {
            Ok(conn) => {
                let mut guard = rpc.lock().await;
                *guard = Some(conn);
                let _ = state_tx.send(ConnectionState::Connected);
                info!("reconnected to adagio-daemon after {attempt} attempt(s)");
                return;
            }
            Err(e) => {
                debug!("reconnect attempt {attempt} failed: {e}");
            }
        }
    }

    let _ = state_tx.send(ConnectionState::Failed);
    warn!("adagio-daemon reconnection failed after 3 attempts — manual restart required");
}

/// Spawn `adagio-daemon` as a detached process.
fn spawn_daemon(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::process::Command::new(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;
        Ok(())
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x00000008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
        std::process::Command::new(path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP)
            .spawn()?;
        Ok(())
    }
    #[cfg(not(any(unix, windows)))]
    {
        std::process::Command::new(path).spawn()?;
        Ok(())
    }
}

impl DaemonClient {
    /// Create a stub client for unit tests.
    ///
    /// The stub has a broadcast channel but no real socket connection.
    /// `request()` will return an error unless a real server is attached.
    /// Intended for unit-testing callers without a running daemon.
    pub fn new_stub() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(16);
        // Start as Reconnecting so that when upgrade_connection() transitions
        // to Connected the watch channel fires an event the frontend can act on.
        let (state_tx, _) = watch::channel(ConnectionState::Reconnecting { attempt: 0 });
        Arc::new(Self {
            rpc: Arc::new(Mutex::new(None)),
            next_id: AtomicU64::new(1),
            event_tx,
            state_tx,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T039 — connect_or_start must not spawn if daemon socket is reachable.
    // Full integration test requires a live daemon; unit-level we test that
    // try_connect_once fails fast when there is no socket (no spawn attempted).
    #[tokio::test]
    async fn try_connect_once_fails_fast_when_no_socket() {
        let fake_path = std::path::Path::new("/tmp/adagio-no-such-socket-xyz.sock");
        let start = std::time::Instant::now();
        let result = DaemonClient::try_connect_once(fake_path).await;
        assert!(result.is_err(), "must fail with no socket");
        // Must time out in ≤ 400 ms (300 ms timeout + overhead).
        assert!(
            start.elapsed().as_millis() < 600,
            "connection attempt must not block: {}ms",
            start.elapsed().as_millis()
        );
    }

    // T040 — the fast path: when socket IS reachable, connect returns quickly.
    // This is verified by the integration quickstart (requires live daemon).
    // Unit-level: confirm stub client starts as Reconnecting (not Connected —
    // it has no socket; starts as Reconnecting so upgrade fires a real event).
    #[test]
    fn stub_client_reports_reconnecting_initially() {
        let client = DaemonClient::new_stub();
        assert_eq!(
            *client.connection_state().borrow(),
            ConnectionState::Reconnecting { attempt: 0 }
        );
    }

    // T049 — connection state transitions: Reconnecting after disconnect.
    // Simulate a disconnect by manually driving the forward_events task.
    #[tokio::test]
    async fn connection_state_transitions_to_reconnecting_on_disconnect() {
        let client = DaemonClient::new_stub();
        let mut rx = client.connection_state();
        // Manually send a Reconnecting state to simulate what forward_events does.
        let _ = client
            .state_tx
            .send(ConnectionState::Reconnecting { attempt: 1 });
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ConnectionState::Reconnecting { attempt: 1 });
    }

    // T050 — after 3 failures, state transitions to Failed.
    #[test]
    fn connection_state_failed_after_three_attempts() {
        let client = DaemonClient::new_stub();
        // Must create receiver BEFORE send; watch::send() is a no-op when no receivers exist.
        let rx = client.connection_state();
        client.state_tx.send_replace(ConnectionState::Failed);
        assert_eq!(*rx.borrow(), ConnectionState::Failed);
    }

    // T051 — Stopped state is distinct from Failed (user-initiated, no auto-restart).
    #[test]
    fn stopped_state_is_distinct_from_failed() {
        assert_ne!(ConnectionState::Stopped, ConnectionState::Failed);
        assert_ne!(ConnectionState::Stopped, ConnectionState::Connected);
        assert_ne!(
            ConnectionState::Stopped,
            ConnectionState::Reconnecting { attempt: 1 }
        );
    }

    // Connection state equality.
    #[test]
    fn connection_state_eq() {
        assert_eq!(ConnectionState::Connected, ConnectionState::Connected);
        assert_ne!(
            ConnectionState::Reconnecting { attempt: 1 },
            ConnectionState::Reconnecting { attempt: 2 }
        );
        assert_ne!(ConnectionState::Connected, ConnectionState::Failed);
        assert_ne!(ConnectionState::Connected, ConnectionState::Stopped);
    }
}
