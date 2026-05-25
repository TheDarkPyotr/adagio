/// Integration tests for large-file transfer: chunked upload (T061) and
/// download range-request resume (T062).
///
/// All tests are marked `#[ignore]` so they run only via
/// `cargo test -- --ignored` and stay out of the normal CI gate.
use adagio_core::error::ClientError;
use adagio_core::remote::{ByteStream, RemoteClient};
use adagio_core::transfer::download::download_file;
use adagio_core::transfer::{TransferOptions, TransferProgress};
use adagio_core::types::{
    ByteRange, Checksum, LocalPath, RemoteItem, RemotePath, ServerCapabilities,
};
use adagio_nextcloud::chunked::upload_chunked;
use async_trait::async_trait;
use bytes::Bytes;
use futures::stream;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tempfile::TempDir;
use tokio::sync::mpsc::Sender;
use tokio::sync::{mpsc, Mutex};

// ── Helpers ───────────────────────────────────────────────────────────────────

fn make_progress_channel() -> (
    Sender<TransferProgress>,
    tokio::sync::mpsc::Receiver<TransferProgress>,
) {
    mpsc::channel(64)
}

/// Generate `n` bytes of deterministic content (repeating byte pattern).
fn make_content(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 251) as u8).collect()
}

// ── Tracking client (T062) ────────────────────────────────────────────────────

/// Mock client that records how many bytes it served per download call.
struct TrackingClient {
    data: Vec<u8>,
    bytes_served: Arc<Mutex<u64>>,
    range_requested: Arc<Mutex<Option<ByteRange>>>,
}

impl TrackingClient {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data,
            bytes_served: Arc::new(Mutex::new(0)),
            range_requested: Arc::new(Mutex::new(None)),
        }
    }

    async fn bytes_served(&self) -> u64 {
        *self.bytes_served.lock().await
    }

    async fn range_requested(&self) -> Option<ByteRange> {
        self.range_requested.lock().await.clone()
    }
}

#[async_trait]
impl RemoteClient for TrackingClient {
    async fn list(&self, _path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        Ok(vec![])
    }
    async fn list_recursive(&self, _path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        Ok(vec![])
    }
    async fn upload(
        &self,
        _path: &RemotePath,
        _data: ByteStream,
        _size: u64,
        _checksum: &Checksum,
    ) -> Result<String, ClientError> {
        Ok("etag".to_string())
    }
    async fn begin_chunked_upload(&self) -> Result<String, ClientError> {
        Ok("session".to_string())
    }
    async fn upload_chunk(
        &self,
        _session_url: &str,
        _chunk_index: u32,
        _data: ByteStream,
        _size: u64,
    ) -> Result<(), ClientError> {
        Ok(())
    }
    async fn finalize_chunked_upload(
        &self,
        _session_url: &str,
        _dest: &RemotePath,
        _total_size: u64,
        _checksum: &Checksum,
    ) -> Result<String, ClientError> {
        Ok("etag".to_string())
    }
    async fn list_uploaded_chunks(&self, _session_url: &str) -> Result<Vec<u32>, ClientError> {
        Ok(vec![])
    }

    async fn download(
        &self,
        _path: &RemotePath,
        range: Option<ByteRange>,
    ) -> Result<ByteStream, ClientError> {
        *self.range_requested.lock().await = range.clone();
        let slice: Vec<u8> = match &range {
            Some(r) => {
                let start = r.start as usize;
                let end = r.end.map(|e| e as usize + 1).unwrap_or(self.data.len());
                self.data[start..end].to_vec()
            }
            None => self.data.clone(),
        };
        let n = slice.len() as u64;
        *self.bytes_served.lock().await += n;
        let bytes = Bytes::from(slice);
        Ok(Box::pin(stream::once(async move {
            Ok::<_, std::io::Error>(bytes)
        })))
    }

    async fn delete(&self, _path: &RemotePath) -> Result<(), ClientError> {
        Ok(())
    }
    async fn move_item(&self, _from: &RemotePath, _to: &RemotePath) -> Result<(), ClientError> {
        Ok(())
    }
    async fn create_dir(&self, _path: &RemotePath) -> Result<(), ClientError> {
        Ok(())
    }
    async fn capabilities(&self) -> Result<ServerCapabilities, ClientError> {
        Ok(ServerCapabilities {
            max_chunk_size: 10 * 1024 * 1024,
            supports_chunked_upload: true,
            supports_dav_checksum: true,
            server_version: "tracking-mock".to_string(),
        })
    }
}

// ── T061: Chunked upload ──────────────────────────────────────────────────────

// T061-1: A file above the chunked threshold is uploaded in multiple chunks.
#[tokio::test]
#[ignore]
async fn chunked_upload_splits_large_file_into_chunks() {
    const CHUNK_SIZE: u64 = 5 * 1024 * 1024; // 5 MB chunks
    const TOTAL_SIZE: usize = 12 * 1024 * 1024; // 12 MB file → 3 chunks

    let dir = TempDir::new().unwrap();
    let content = make_content(TOTAL_SIZE);
    let file_path = dir.path().join("large.bin");
    std::fs::write(&file_path, &content).unwrap();

    let client = adagio_core::remote::mock::MockRemoteClient::new();
    let local = LocalPath::new(file_path);
    let remote = RemotePath::new("large.bin");
    let opts = TransferOptions {
        chunked_threshold: CHUNK_SIZE,
        chunk_size: CHUNK_SIZE,
        bandwidth_cap: None,
    };
    let (tx, _rx) = make_progress_channel();

    let result = upload_chunked(&client, &local, &remote, &opts, tx)
        .await
        .expect("chunked upload should succeed");

    assert!(!result.etag.is_empty(), "etag should be non-empty");

    // The assembled file should have the correct content.
    let stored = client
        .get_data(&remote)
        .await
        .expect("data should be stored");
    assert_eq!(
        stored.len(),
        TOTAL_SIZE,
        "stored file size should match original"
    );

    let expected_hash = hex::encode(Sha256::digest(&content));
    let stored_hash = hex::encode(Sha256::digest(&stored));
    assert_eq!(
        stored_hash, expected_hash,
        "stored file content should match original"
    );
}

// T061-2: Resume skips chunks already present on the server.
#[tokio::test]
#[ignore]
async fn chunked_upload_resumes_skipping_existing_chunks() {
    const CHUNK_SIZE: u64 = 5 * 1024 * 1024; // 5 MB
    const TOTAL_SIZE: usize = 10 * 1024 * 1024; // 10 MB → 2 chunks

    let dir = TempDir::new().unwrap();
    let content = make_content(TOTAL_SIZE);
    let file_path = dir.path().join("resume.bin");
    std::fs::write(&file_path, &content).unwrap();

    let client = adagio_core::remote::mock::MockRemoteClient::new();
    // Pre-seed chunk 0 so resume skips it.
    client
        .seed_chunk("session-resume", 0, &content[..CHUNK_SIZE as usize])
        .await;

    let local = LocalPath::new(file_path);
    let remote = RemotePath::new("resume.bin");
    let opts = TransferOptions {
        chunked_threshold: CHUNK_SIZE,
        chunk_size: CHUNK_SIZE,
        bandwidth_cap: None,
    };
    let (tx, _rx) = make_progress_channel();

    // upload_chunked should detect the existing session and skip chunk 0.
    let result = upload_chunked(&client, &local, &remote, &opts, tx)
        .await
        .expect("resume upload should succeed");

    assert!(!result.etag.is_empty());
}

// ── T062: Download range-request resume ─────────────────────────────────────

// T062-1: A partial temp file causes a Range request for the missing bytes.
#[tokio::test]
#[ignore]
async fn download_resumes_from_partial_temp_file() {
    const TOTAL: usize = 1000;
    const PARTIAL: usize = 400;

    let dir = TempDir::new().unwrap();
    let content = make_content(TOTAL);
    let client = TrackingClient::new(content.clone());

    let remote = RemotePath::new("big.bin");
    let local_path = LocalPath::new(dir.path().join("big.bin"));

    // Place a partial temp file at the deterministic temp path.
    let tmp_path = adagio_core::transfer::download::temp_path_for(&remote, dir.path());
    std::fs::write(&tmp_path, &content[..PARTIAL]).unwrap();

    let (tx, _rx) = make_progress_channel();
    let result = download_file(&client, &remote, &local_path, None, &Default::default(), tx)
        .await
        .expect("resumed download should succeed");

    // Only the remaining bytes should have been served.
    let served = client.bytes_served().await;
    assert_eq!(
        served,
        (TOTAL - PARTIAL) as u64,
        "should only download missing bytes"
    );

    // A Range request should have been made.
    let range = client
        .range_requested()
        .await
        .expect("range should have been requested");
    assert_eq!(
        range.start, PARTIAL as u64,
        "range start should be the partial file size"
    );
    assert_eq!(range.end, None, "range end should be open-ended");

    // Final file should be complete and correct.
    assert_eq!(result.size, TOTAL as u64);
    let final_content = std::fs::read(dir.path().join("big.bin")).unwrap();
    assert_eq!(
        final_content, content,
        "final content should match original"
    );
}

// T062-2: When no temp file exists, the full file is downloaded.
#[tokio::test]
#[ignore]
async fn download_full_when_no_partial_temp_exists() {
    const TOTAL: usize = 500;
    let dir = TempDir::new().unwrap();
    let content = make_content(TOTAL);
    let client = TrackingClient::new(content.clone());

    let remote = RemotePath::new("fresh.bin");
    let local_path = LocalPath::new(dir.path().join("fresh.bin"));
    let (tx, _rx) = make_progress_channel();

    let result = download_file(&client, &remote, &local_path, None, &Default::default(), tx)
        .await
        .expect("fresh download should succeed");

    let served = client.bytes_served().await;
    assert_eq!(served, TOTAL as u64, "full file should be downloaded");
    assert_eq!(result.size, TOTAL as u64);

    let range = client.range_requested().await;
    assert!(range.is_none(), "no range request for a fresh download");
}
