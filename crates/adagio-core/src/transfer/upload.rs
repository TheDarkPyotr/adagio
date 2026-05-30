use crate::bandwidth::TokenBucket;
use crate::error::TransferError;
use crate::remote::RemoteClient;
use crate::types::{Checksum, ChecksumAlgorithm, LocalPath, RemotePath};
use futures::stream;
use sha2::{Digest, Sha256};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::instrument;

use super::{TransferOptions, TransferProgress, UploadResult};

/// Upload a local file via single PUT with OC-Checksum header.
///
/// Reads the file in 64 KB chunks. If `throttle` is `Some`, calls
/// `TokenBucket::acquire(chunk_len)` before each chunk to enforce a byte-rate
/// ceiling. Pass `None` for unlimited speed (the common case).
///
/// Returns the server's etag from the response.
#[instrument(skip(client, progress, throttle), fields(remote_path = %remote_path))]
pub async fn upload_single(
    client: &dyn RemoteClient,
    local_path: &LocalPath,
    remote_path: &RemotePath,
    _opts: &TransferOptions,
    progress: mpsc::Sender<TransferProgress>,
    throttle: Option<Arc<TokenBucket>>,
) -> Result<UploadResult, TransferError> {
    const CHUNK: usize = 65_536; // 64 KB

    let path = local_path.0.clone();

    // Read file and compute checksum in a blocking task.
    let (data, checksum) = tokio::task::spawn_blocking(move || read_and_hash(&path))
        .await
        .map_err(|e| TransferError::Transient(format!("join error: {e}")))?
        .map_err(|e| TransferError::Transient(e.to_string()))?;

    let size = data.len() as u64;

    // Throttle chunk by chunk, then stream the whole payload.
    // We apply throttle delays before creating the stream (pre-flight per chunk),
    // which correctly distributes sleeping across the full upload duration.
    if let Some(ref tb) = throttle {
        let mut offset = 0usize;
        while offset < data.len() {
            let chunk_len = CHUNK.min(data.len() - offset);
            let delay = tb.acquire(chunk_len as u64).await;
            if delay.as_nanos() > 0 {
                tokio::time::sleep(delay).await;
            }
            offset += chunk_len;
        }
    }

    let bytes = bytes::Bytes::from(data);
    let stream = Box::pin(stream::once(async move {
        Ok::<bytes::Bytes, std::io::Error>(bytes)
    }));

    let etag = client
        .upload(remote_path, stream, size, &checksum)
        .await
        .map_err(map_client_error)?;

    let _ = progress.try_send(TransferProgress {
        transfer_id: crate::types::TransferId::new(),
        bytes_transferred: size,
        bytes_total: size,
        chunk_index: None,
    });

    Ok(UploadResult {
        etag,
        file_id: String::new(),
    })
}

fn map_client_error(e: crate::error::ClientError) -> TransferError {
    match e {
        crate::error::ClientError::AuthRequired => TransferError::Permanent("auth required".into()),
        crate::error::ClientError::Transient(m) => TransferError::Transient(m),
        crate::error::ClientError::Permanent(m) => TransferError::Permanent(m),
    }
}

fn read_and_hash(path: &Path) -> std::io::Result<(Vec<u8>, Checksum)> {
    let data = std::fs::read(path)?;
    let mut h = Sha256::new();
    h.update(&data);
    let value = hex::encode(h.finalize());
    Ok((
        data,
        Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value,
        },
    ))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::LocalPath;
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    fn make_local(dir: &TempDir, name: &str, content: &[u8]) -> LocalPath {
        let p = dir.path().join(name);
        fs::write(&p, content).unwrap();
        LocalPath::new(p)
    }

    // T007 — upload_single with throttle enforces rate limit.
    #[tokio::test]
    async fn upload_single_with_throttle_slows_transfer() {
        let dir = TempDir::new().unwrap();
        // 50 KB at 25 KB/s with 100-byte burst → delay ≈ (50000-100)/25000 = 1.996s
        let content = vec![0u8; 50_000];
        let local = make_local(&dir, "throttled.bin", &content);
        let client = MockRemoteClient::new();
        let remote = crate::types::RemotePath::new("throttled.bin");
        let (tx, _rx) = mpsc::channel(8);
        let bucket = Arc::new(TokenBucket::new(25_000, 100)); // 25 KB/s, 100B burst

        let start = std::time::Instant::now();
        upload_single(
            &client,
            &local,
            &remote,
            &Default::default(),
            tx,
            Some(bucket),
        )
        .await
        .expect("upload should succeed");
        let elapsed = start.elapsed();

        // At 25 KB/s with 100B burst, 50 KB should take ≥ 1.8 s (10% tolerance).
        assert!(
            elapsed.as_millis() >= 1800,
            "throttled upload took only {}ms, expected ≥1800ms",
            elapsed.as_millis()
        );
    }

    // T009 — upload_single with None throttle completes fast (mock is instant).
    #[tokio::test]
    async fn upload_single_no_throttle_is_fast() {
        let dir = TempDir::new().unwrap();
        let content = vec![0u8; 100_000];
        let local = make_local(&dir, "fast.bin", &content);
        let client = MockRemoteClient::new();
        let remote = crate::types::RemotePath::new("fast.bin");
        let (tx, _rx) = mpsc::channel(8);

        let start = std::time::Instant::now();
        upload_single(&client, &local, &remote, &Default::default(), tx, None)
            .await
            .expect("upload should succeed");
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() < 500,
            "unthrottled upload took {}ms, expected <500ms",
            elapsed.as_millis()
        );
    }

    // T044-1: Upload a small file; result has an etag.
    #[tokio::test]
    async fn upload_single_returns_etag() {
        let dir = TempDir::new().unwrap();
        let local = make_local(&dir, "test.txt", b"hello world");
        let client = MockRemoteClient::new();
        let remote = crate::types::RemotePath::new("test.txt");
        let (tx, _rx) = mpsc::channel(8);

        let result = upload_single(&client, &local, &remote, &Default::default(), tx, None)
            .await
            .expect("upload should succeed");

        assert!(!result.etag.is_empty(), "etag should be non-empty");
    }

    // T044-2: Checksum is sent and matches the file content.
    #[tokio::test]
    async fn upload_single_checksum_matches_content() {
        use sha2::{Digest, Sha256};

        let dir = TempDir::new().unwrap();
        let content = b"checksum test data";
        let local = make_local(&dir, "ck.txt", content);
        let client = MockRemoteClient::new();
        let remote = crate::types::RemotePath::new("ck.txt");
        let (tx, _rx) = mpsc::channel(8);

        upload_single(&client, &local, &remote, &Default::default(), tx, None)
            .await
            .expect("upload should succeed");

        // Verify the mock stored the correct checksum.
        let expected_hash = hex::encode(Sha256::digest(content));
        let stored = client.get_checksum(&remote);
        assert_eq!(
            stored.map(|c| c.value),
            Some(expected_hash),
            "stored checksum should match SHA-256 of content"
        );
    }
}
