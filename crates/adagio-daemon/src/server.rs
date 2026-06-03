use adagio_ipc::{DaemonEvent, DaemonRequest};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use crate::dispatcher::{dispatch, DaemonProcess};

/// Accept IPC connections on the Unix socket and dispatch requests.
///
/// Runs until the listener is closed or the process exits.
#[cfg(unix)]
pub async fn accept_loop(
    listener: tokio::net::UnixListener,
    state: Arc<DaemonProcess>,
    event_rx_factory: impl Fn() -> broadcast::Receiver<DaemonEvent> + Send + Sync + 'static,
) {
    let factory = Arc::new(event_rx_factory);
    loop {
        match listener.accept().await {
            Ok((stream, _addr)) => {
                let state = state.clone();
                let rx = factory();
                info!("new IPC connection accepted");
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, state, rx).await {
                        debug!("IPC connection closed: {e}");
                    }
                });
            }
            Err(e) => {
                warn!("accept error: {e}");
                break;
            }
        }
    }
}

/// Handle a single IPC connection.
///
/// Reads the first line to determine whether this is an RPC connection or
/// a subscription connection, then delegates accordingly.
#[cfg(unix)]
async fn handle_connection(
    stream: tokio::net::UnixStream,
    state: Arc<DaemonProcess>,
    event_rx: broadcast::Receiver<DaemonEvent>,
) -> std::io::Result<()> {
    let (read_half, write_half) = stream.into_split();
    let mut reader = BufReader::new(read_half);
    let mut writer = write_half;

    let mut first_line = String::new();
    reader.read_line(&mut first_line).await?;
    let first_line = first_line.trim_end_matches('\n').trim_end_matches('\r');

    // Detect subscription connection.
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(first_line) {
        if v.get("type").and_then(|t| t.as_str()) == Some("subscribe") {
            return handle_subscription(writer, event_rx).await;
        }
    }

    // Otherwise handle as RPC: process the first line we already read, then loop.
    handle_rpc_line(first_line, &mut writer, &state).await?;
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).await?;
        if n == 0 {
            break; // EOF
        }
        handle_rpc_line(
            line.trim_end_matches('\n').trim_end_matches('\r'),
            &mut writer,
            &state,
        )
        .await?;
    }
    Ok(())
}

#[cfg(unix)]
async fn handle_rpc_line(
    line: &str,
    writer: &mut tokio::net::unix::OwnedWriteHalf,
    state: &DaemonProcess,
) -> std::io::Result<()> {
    if line.is_empty() {
        return Ok(());
    }

    // Parse as `{"id": N, "method": "...", "params": {...}}`
    let parsed: serde_json::Value = match serde_json::from_str(line) {
        Ok(v) => v,
        Err(e) => {
            let err = serde_json::json!({"id": null, "error": {"code": -32700, "message": format!("parse error: {e}")}});
            writer.write_all(format!("{err}\n").as_bytes()).await?;
            return Ok(());
        }
    };

    let id = parsed.get("id").cloned().unwrap_or(serde_json::Value::Null);
    let method = parsed["method"].as_str().unwrap_or("").to_string();
    let params = parsed
        .get("params")
        .cloned()
        .unwrap_or(serde_json::json!({}));

    // Re-compose a DaemonRequest value and deserialise.
    let req_val = serde_json::json!({"method": method, "params": params});
    let req: DaemonRequest = match serde_json::from_value(req_val) {
        Ok(r) => r,
        Err(e) => {
            let err = serde_json::json!({"id": id, "error": {"code": -32601, "message": format!("method not found: {e}")}});
            writer.write_all(format!("{err}\n").as_bytes()).await?;
            return Ok(());
        }
    };

    // Check for stop_daemon — tell the accept loop to shut down.
    let is_stop = matches!(req, DaemonRequest::StopDaemon);

    let response = dispatch(req, state).await;
    let reply = match response {
        Ok(result) => serde_json::json!({"id": id, "result": result}),
        Err(msg) => serde_json::json!({"id": id, "error": {"code": -32000, "message": msg}}),
    };
    writer.write_all(format!("{reply}\n").as_bytes()).await?;

    if is_stop {
        return Err(std::io::Error::other("stop_daemon"));
    }
    Ok(())
}

#[cfg(unix)]
async fn handle_subscription(
    mut writer: tokio::net::unix::OwnedWriteHalf,
    mut rx: broadcast::Receiver<DaemonEvent>,
) -> std::io::Result<()> {
    loop {
        match rx.recv().await {
            Ok(event) => {
                let line = serde_json::to_string(&event).unwrap_or_default();
                writer.write_all(format!("{line}\n").as_bytes()).await?;
                // Stop forwarding after ShuttingDown event.
                if matches!(event, DaemonEvent::ShuttingDown { .. }) {
                    break;
                }
            }
            Err(broadcast::error::RecvError::Closed) => break,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                debug!("subscription lagged, dropped {n} events");
            }
        }
    }
    Ok(())
}
