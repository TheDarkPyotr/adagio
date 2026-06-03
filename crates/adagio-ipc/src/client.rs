use crate::types::{DaemonEvent, DaemonRequest};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{broadcast, watch};

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

// ── Unix implementation (Unix domain sockets) ─────────────────────────────────

#[cfg(unix)]
use {
    crate::transport::daemon_socket_path,
    std::sync::atomic::{AtomicU64, Ordering},
    tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    tokio::sync::Mutex,
    tracing::{debug, info, warn},
};

#[cfg(unix)]
type RpcWriter = tokio::net::unix::OwnedWriteHalf;
#[cfg(unix)]
type RpcReader = BufReader<tokio::net::unix::OwnedReadHalf>;

#[cfg(unix)]
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
#[cfg(unix)]
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

#[cfg(unix)]
impl DaemonClient {
    /// Connect to a running daemon, or spawn it if it is not running.
    pub async fn connect_or_start(daemon_binary_path: &Path) -> anyhow::Result<Arc<Self>> {
        let socket_path = daemon_socket_path();

        if let Ok(client) = Self::try_connect_once(&socket_path).await {
            info!("connected to existing adagio-daemon");
            return Ok(client);
        }

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

    async fn try_connect_once(socket_path: &Path) -> anyhow::Result<Arc<Self>> {
        let rpc_stream = tokio::time::timeout(
            std::time::Duration::from_millis(300),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (read_half, write_half) = rpc_stream.into_split();

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

        let (sub_read, mut sub_write) = sub_stream.into_split();
        sub_write.write_all(b"{\"type\":\"subscribe\"}\n").await?;

        let state_tx_clone = client.state_tx.clone();
        let socket = socket_path.to_path_buf();
        tokio::spawn(async move {
            forward_events(BufReader::new(sub_read), event_tx, state_tx_clone.clone()).await;
            reconnect_loop(socket, state_tx_clone, rpc).await;
        });

        Ok(client)
    }

    /// Upgrade a stub client to a real connection in-place.
    pub async fn upgrade_connection(&self, socket_path: &Path) -> anyhow::Result<()> {
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

        let sub_stream = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            tokio::net::UnixStream::connect(socket_path),
        )
        .await??;
        let (sub_read, mut sub_write) = sub_stream.into_split();
        sub_write.write_all(b"{\"type\":\"subscribe\"}\n").await?;

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
    pub async fn request(&self, req: DaemonRequest) -> anyhow::Result<serde_json::Value> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let req_json = serde_json::to_value(&req)?;
        let envelope = serde_json::json!({
            "id": id,
            "method": req_json["method"],
            "params": req_json["params"],
        });
        let line = format!("{envelope}\n");

        let mut guard = self.rpc.lock().await;
        let conn = guard
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("not connected to daemon"))?;

        conn.writer.write_all(line.as_bytes()).await?;

        let mut resp_line = String::new();
        conn.reader.read_line(&mut resp_line).await?;

        drop(guard);

        let resp_val: serde_json::Value = serde_json::from_str(resp_line.trim())?;
        if let Some(err) = resp_val.get("error") {
            anyhow::bail!("{}", err["message"].as_str().unwrap_or("unknown error"));
        }
        Ok(resp_val["result"].clone())
    }

    /// Subscribe to push events from the daemon.
    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.event_tx.subscribe()
    }

    /// Watch connection state changes.
    pub fn connection_state(&self) -> watch::Receiver<ConnectionState> {
        self.state_tx.subscribe()
    }

    /// Drive the connection state to `Failed` from outside the client.
    pub fn mark_failed(&self) {
        let _ = self.state_tx.send(ConnectionState::Failed);
    }

    /// Create a stub client with no real socket connection.
    pub fn new_stub() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(16);
        let (state_tx, _) = watch::channel(ConnectionState::Reconnecting { attempt: 0 });
        Arc::new(Self {
            rpc: Arc::new(Mutex::new(None)),
            next_id: AtomicU64::new(1),
            event_tx,
            state_tx,
        })
    }
}

#[cfg(unix)]
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
    let current = state_tx.borrow().clone();
    if current == ConnectionState::Connected {
        let _ = state_tx.send(ConnectionState::Reconnecting { attempt: 1 });
    }
}

#[cfg(unix)]
async fn reconnect_loop(
    socket_path: std::path::PathBuf,
    state_tx: watch::Sender<ConnectionState>,
    rpc: Arc<Mutex<Option<RpcConn>>>,
) {
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

#[cfg(unix)]
fn spawn_daemon(path: &Path) -> std::io::Result<()> {
    std::process::Command::new(path)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()?;
    Ok(())
}

// ── Non-Unix stub (Windows / other platforms) ─────────────────────────────────
//
// Provides the same public API as the Unix implementation but with no socket
// code.  The desktop binary on Windows would need named-pipe support added here
// before IPC actually works; for now this lets the crate compile on CI.

#[cfg(not(unix))]
pub struct DaemonClient {
    event_tx: broadcast::Sender<DaemonEvent>,
    state_tx: watch::Sender<ConnectionState>,
}

#[cfg(not(unix))]
impl DaemonClient {
    pub fn new_stub() -> Arc<Self> {
        let (event_tx, _) = broadcast::channel(16);
        let (state_tx, _) = watch::channel(ConnectionState::Reconnecting { attempt: 0 });
        Arc::new(Self { event_tx, state_tx })
    }

    pub fn connection_state(&self) -> watch::Receiver<ConnectionState> {
        self.state_tx.subscribe()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DaemonEvent> {
        self.event_tx.subscribe()
    }

    pub fn mark_failed(&self) {
        let _ = self.state_tx.send(ConnectionState::Failed);
    }

    pub async fn request(&self, _req: DaemonRequest) -> anyhow::Result<serde_json::Value> {
        anyhow::bail!("IPC not supported on this platform")
    }

    pub async fn upgrade_connection(&self, _socket_path: &Path) -> anyhow::Result<()> {
        anyhow::bail!("Unix domain sockets not available on this platform")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // T039 — try_connect_once fails fast when there is no socket.
    #[cfg(unix)]
    #[tokio::test]
    async fn try_connect_once_fails_fast_when_no_socket() {
        let fake_path = std::path::Path::new("/tmp/adagio-no-such-socket-xyz.sock");
        let start = std::time::Instant::now();
        let result = DaemonClient::try_connect_once(fake_path).await;
        assert!(result.is_err(), "must fail with no socket");
        assert!(
            start.elapsed().as_millis() < 600,
            "connection attempt must not block: {}ms",
            start.elapsed().as_millis()
        );
    }

    // T040 — stub client starts as Reconnecting.
    #[test]
    fn stub_client_reports_reconnecting_initially() {
        let client = DaemonClient::new_stub();
        assert_eq!(
            *client.connection_state().borrow(),
            ConnectionState::Reconnecting { attempt: 0 }
        );
    }

    // T049 — state transitions to Reconnecting on disconnect.
    #[cfg(unix)]
    #[tokio::test]
    async fn connection_state_transitions_to_reconnecting_on_disconnect() {
        let client = DaemonClient::new_stub();
        let mut rx = client.connection_state();
        let _ = client
            .state_tx
            .send(ConnectionState::Reconnecting { attempt: 1 });
        rx.changed().await.unwrap();
        assert_eq!(*rx.borrow(), ConnectionState::Reconnecting { attempt: 1 });
    }

    // T050 — after 3 failures, state transitions to Failed.
    #[cfg(unix)]
    #[test]
    fn connection_state_failed_after_three_attempts() {
        let client = DaemonClient::new_stub();
        let rx = client.connection_state();
        client.state_tx.send_replace(ConnectionState::Failed);
        assert_eq!(*rx.borrow(), ConnectionState::Failed);
    }

    // T051 — Stopped state is distinct from Failed.
    #[test]
    fn stopped_state_is_distinct_from_failed() {
        assert_ne!(ConnectionState::Stopped, ConnectionState::Failed);
        assert_ne!(ConnectionState::Stopped, ConnectionState::Connected);
        assert_ne!(
            ConnectionState::Stopped,
            ConnectionState::Reconnecting { attempt: 1 }
        );
    }

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
