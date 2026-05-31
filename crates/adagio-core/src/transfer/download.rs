use crate::bandwidth::TokenBucket;
use crate::error::TransferError;
use crate::remote::RemoteClient;
use crate::types::{ByteRange, Checksum, ChecksumAlgorithm, LocalPath, RemotePath};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use tracing::instrument;

use super::{DownloadResult, TransferOptions, TransferProgress};

/// Returns a deterministic temp file path for `remote_path` in `dest_dir`.
///
/// The name is derived from a hash of the remote path string so that an
/// interrupted download can be resumed: the same temp file is found on the
/// next call and its existing size becomes the Range start.
pub fn temp_path_for(remote_path: &RemotePath, dest_dir: &Path) -> PathBuf {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    remote_path.0.hash(&mut h);
    dest_dir.join(format!(".adagio_dl_{:016x}", h.finish()))
}

/// Download a remote file to a deterministic temp file, verify checksum, then
/// atomically rename to the final destination (T066).
///
/// If a partial temp file already exists (from a prior interrupted download),
/// the download resumes from that offset using a `Range: bytes=<offset>-`
/// request. The partial data is read back and fed into the SHA-256 hasher so
/// the final checksum covers the entire file.
///
/// On checksum mismatch the temp file is deleted and `TransferError::Transient` is returned.
/// On disk-full (`StorageFull`) the temp file is deleted and `TransferError::Permanent` is returned.
#[instrument(skip(client, progress, throttle), fields(remote_path = %remote_path))]
pub async fn download_file(
    client: &dyn RemoteClient,
    remote_path: &RemotePath,
    local_path: &LocalPath,
    expected_checksum: Option<Checksum>,
    _opts: &TransferOptions,
    progress: tokio::sync::mpsc::Sender<TransferProgress>,
    throttle: Option<Arc<TokenBucket>>,
) -> Result<DownloadResult, TransferError> {
    let dest: PathBuf = local_path.0.clone();
    let parent = dest.parent().unwrap_or(Path::new(".")).to_path_buf();
    let tmp_path = temp_path_for(remote_path, &parent);

    // Ensure parent directory exists.
    tokio::fs::create_dir_all(&parent)
        .await
        .map_err(map_io_error)?;

    // Check for an existing partial download.
    let existing_size = match tokio::fs::metadata(&tmp_path).await {
        Ok(m) => m.len(),
        Err(_) => 0,
    };

    let mut hasher = Sha256::new();
    let mut total_bytes: u64 = existing_size;

    // Open temp file: append if resuming, create fresh otherwise.
    let mut file = if existing_size > 0 {
        // Re-hash the already-downloaded bytes so the final hash is correct.
        let existing_data = tokio::fs::read(&tmp_path).await.map_err(map_io_error)?;
        hasher.update(&existing_data);

        tokio::fs::OpenOptions::new()
            .append(true)
            .open(&tmp_path)
            .await
            .map_err(map_io_error)?
    } else {
        tokio::fs::File::create(&tmp_path)
            .await
            .map_err(map_io_error)?
    };

    // Issue full or range request depending on whether we're resuming.
    let range = if existing_size > 0 {
        Some(ByteRange {
            start: existing_size,
            end: None,
        })
    } else {
        None
    };

    let stream = client
        .download(remote_path, range)
        .await
        .map_err(|e| match e {
            crate::error::ClientError::AuthRequired => {
                TransferError::Permanent("auth required".into())
            }
            crate::error::ClientError::Transient(m) => TransferError::Transient(m),
            crate::error::ClientError::Permanent(m) => TransferError::Permanent(m),
            crate::error::ClientError::Maintenance => {
                TransferError::Transient("server in maintenance mode".into())
            }
        })?;

    let mut stream = stream;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            TransferError::Transient(e.to_string())
        })?;

        // Throttle: sleep before writing each chunk.
        if let Some(ref tb) = throttle {
            let delay = tb.acquire(chunk.len() as u64).await;
            if delay.as_nanos() > 0 {
                tokio::time::sleep(delay).await;
            }
        }

        hasher.update(&chunk);
        total_bytes += chunk.len() as u64;

        file.write_all(&chunk).await.map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            map_io_error(e)
        })?;
    }

    file.flush().await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        map_io_error(e)
    })?;
    drop(file);

    let hash_value = hex::encode(hasher.finalize());
    let actual_checksum = Checksum {
        algorithm: ChecksumAlgorithm::Sha256,
        value: hash_value,
    };

    // Verify checksum if provided.
    if let Some(expected) = &expected_checksum {
        if expected.algorithm == actual_checksum.algorithm
            && expected.value != actual_checksum.value
        {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(TransferError::Transient(format!(
                "checksum mismatch: expected {}, got {}",
                expected.value, actual_checksum.value
            )));
        }
    }

    // Atomic rename to final destination.
    tokio::fs::rename(&tmp_path, &dest).await.map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        map_io_error(e)
    })?;

    let _ = progress.try_send(TransferProgress {
        transfer_id: crate::types::TransferId::new(),
        bytes_transferred: total_bytes,
        bytes_total: total_bytes,
        chunk_index: None,
    });

    Ok(DownloadResult {
        checksum: actual_checksum,
        size: total_bytes,
    })
}

#[allow(clippy::incompatible_msrv)]
fn map_io_error(e: std::io::Error) -> TransferError {
    if e.kind() == std::io::ErrorKind::StorageFull {
        TransferError::Permanent("disk full".to_string())
    } else {
        TransferError::Transient(e.to_string())
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::{LocalPath, RemotePath};
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // T008 — download_file with throttle enforces rate limit.
    #[tokio::test]
    async fn download_file_with_throttle_slows_transfer() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        // 50 KB content; 25 KB/s, 100-byte burst → ≥ ~2 s
        let content = vec![42u8; 50_000];
        client.seed("throttled.bin", &content).await;

        let remote = RemotePath::new("throttled.bin");
        let local = LocalPath::new(dir.path().join("throttled.bin"));
        let (tx, _rx) = mpsc::channel(8);
        let bucket = Arc::new(TokenBucket::new(25_000, 100)); // 25 KB/s, tiny burst

        let start = std::time::Instant::now();
        download_file(
            &client,
            &remote,
            &local,
            None,
            &Default::default(),
            tx,
            Some(bucket),
        )
        .await
        .expect("download should succeed");
        let elapsed = start.elapsed();

        assert!(
            elapsed.as_millis() >= 1800,
            "throttled download took only {}ms, expected ≥1800ms",
            elapsed.as_millis()
        );
    }

    // T045-1: Download creates local file with correct content.
    #[tokio::test]
    async fn download_creates_local_file() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("doc.txt", b"hello download").await;

        let remote = RemotePath::new("doc.txt");
        let local = LocalPath::new(dir.path().join("doc.txt"));
        let (tx, _rx) = mpsc::channel(8);

        let result = download_file(
            &client,
            &remote,
            &local,
            None,
            &Default::default(),
            tx,
            None,
        )
        .await
        .expect("download should succeed");

        assert_eq!(result.size, 14);
        let content = std::fs::read(dir.path().join("doc.txt")).unwrap();
        assert_eq!(content, b"hello download");
    }

    // T045-2: Checksum is computed and returned.
    #[tokio::test]
    async fn download_returns_correct_checksum() {
        let dir = TempDir::new().unwrap();
        let content = b"checksum content";
        let client = MockRemoteClient::new();
        client.seed("ck.txt", content).await;

        let remote = RemotePath::new("ck.txt");
        let local = LocalPath::new(dir.path().join("ck.txt"));
        let (tx, _rx) = mpsc::channel(8);

        let result = download_file(
            &client,
            &remote,
            &local,
            None,
            &Default::default(),
            tx,
            None,
        )
        .await
        .unwrap();

        let expected_hash = hex::encode(Sha256::digest(content));
        assert_eq!(result.checksum.value, expected_hash);
    }

    // T045-3: Checksum mismatch causes temp file deletion and error.
    #[tokio::test]
    async fn download_rejects_checksum_mismatch() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("bad.txt", b"real content").await;

        let remote = RemotePath::new("bad.txt");
        let local = LocalPath::new(dir.path().join("bad.txt"));
        let wrong_checksum = Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: "0".repeat(64),
        };
        let (tx, _rx) = mpsc::channel(8);

        let err = download_file(
            &client,
            &remote,
            &local,
            Some(wrong_checksum),
            &Default::default(),
            tx,
            None,
        )
        .await
        .expect_err("should fail on checksum mismatch");

        assert!(matches!(err, TransferError::Transient(_)));
        // Final file should NOT exist.
        assert!(
            !dir.path().join("bad.txt").exists(),
            "partial file should be cleaned up"
        );
    }

    // T078: StorageFull IO error maps to Permanent, other IO errors map to Transient.
    #[test]
    fn disk_full_maps_to_permanent() {
        let e = std::io::Error::new(std::io::ErrorKind::StorageFull, "no space left");
        assert!(matches!(map_io_error(e), TransferError::Permanent(_)));
    }

    #[test]
    fn other_io_error_maps_to_transient() {
        let e = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "broken pipe");
        assert!(matches!(map_io_error(e), TransferError::Transient(_)));
    }

    // T045-4: Temp file is not left behind on success.
    #[tokio::test]
    async fn download_no_tmp_file_after_success() {
        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        client.seed("clean.txt", b"data").await;

        let remote = RemotePath::new("clean.txt");
        let local = LocalPath::new(dir.path().join("clean.txt"));
        let (tx, _rx) = mpsc::channel(8);

        download_file(
            &client,
            &remote,
            &local,
            None,
            &Default::default(),
            tx,
            None,
        )
        .await
        .unwrap();

        // No .adagio_tmp_* files should remain.
        let tmp_count = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with(".adagio_tmp_"))
            .count();
        assert_eq!(
            tmp_count, 0,
            "no temp files should remain after successful download"
        );
    }
}
