use crate::error::TransferError;
use crate::remote::RemoteClient;
use crate::types::{Checksum, ChecksumAlgorithm, LocalPath, RemotePath};
use futures::stream;
use sha2::{Digest, Sha256};
use std::path::Path;
use tokio::sync::mpsc;
use tracing::instrument;

use super::{TransferOptions, TransferProgress, UploadResult};

/// Upload a local file via single PUT with OC-Checksum header.
///
/// Reads the file, computes its SHA-256, then streams the bytes to the remote.
/// Returns the server's etag from the response.
///
/// For files above `opts.chunked_threshold`, use `upload_chunked` instead.
#[instrument(skip(client, progress), fields(remote_path = %remote_path))]
pub async fn upload_single(
    client: &dyn RemoteClient,
    local_path: &LocalPath,
    remote_path: &RemotePath,
    _opts: &TransferOptions,
    progress: mpsc::Sender<TransferProgress>,
) -> Result<UploadResult, TransferError> {
    let path = local_path.0.clone();

    // Read file and compute checksum in a blocking task.
    let (data, checksum) = tokio::task::spawn_blocking(move || read_and_hash(&path))
        .await
        .map_err(|e| TransferError::Transient(format!("join error: {e}")))?
        .map_err(|e| TransferError::Transient(e.to_string()))?;

    let size = data.len() as u64;
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

    // T044-1: Upload a small file; result has an etag.
    #[tokio::test]
    async fn upload_single_returns_etag() {
        let dir = TempDir::new().unwrap();
        let local = make_local(&dir, "test.txt", b"hello world");
        let client = MockRemoteClient::new();
        let remote = crate::types::RemotePath::new("test.txt");
        let (tx, _rx) = mpsc::channel(8);

        let result = upload_single(&client, &local, &remote, &Default::default(), tx)
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

        upload_single(&client, &local, &remote, &Default::default(), tx)
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
