use crate::error::ClientError;
use crate::remote::{ByteStream, RemoteClient};
use crate::types::{ByteRange, Checksum, RemoteItem, RemotePath, ServerCapabilities};
use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use futures::stream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Clone, Default)]
struct Item {
    data: Vec<u8>,
    is_dir: bool,
    etag: String,
    file_id: String,
    checksum: Option<Checksum>,
}

/// In-memory `RemoteClient` for use in contract tests and unit tests.
#[derive(Clone, Default)]
pub struct MockRemoteClient {
    items: Arc<RwLock<HashMap<String, Item>>>,
    /// Stores chunks during a chunked upload: key = (session, chunk_index), value = bytes.
    #[allow(clippy::type_complexity)]
    chunks: Arc<RwLock<HashMap<(String, u32), Vec<u8>>>>,
    capabilities: ServerCapabilities,
    /// Counts calls to `upload()` (single-PUT path).
    upload_calls: Arc<std::sync::atomic::AtomicUsize>,
    /// Counts calls to `begin_chunked_upload()`.
    begin_chunked_calls: Arc<std::sync::atomic::AtomicUsize>,
}

impl MockRemoteClient {
    pub fn new() -> Self {
        Self {
            items: Arc::new(RwLock::new(HashMap::new())),
            chunks: Arc::new(RwLock::new(HashMap::new())),
            capabilities: ServerCapabilities {
                max_chunk_size: 10 * 1024 * 1024,
                supports_chunked_upload: true,
                supports_dav_checksum: true,
                server_version: "mock-1.0".into(),
            },
            upload_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            begin_chunked_calls: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        }
    }

    /// Number of single-PUT `upload()` calls received.
    pub fn upload_call_count(&self) -> usize {
        self.upload_calls.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Number of `begin_chunked_upload()` calls received.
    pub fn begin_chunked_upload_call_count(&self) -> usize {
        self.begin_chunked_calls
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Seed a file directly (bypasses upload).
    pub async fn seed(&self, path: &str, data: &[u8]) {
        let mut items = self.items.write().await;
        items.insert(
            path.to_string(),
            Item {
                data: data.to_vec(),
                is_dir: false,
                etag: format!("{:016x}", data.len()),
                file_id: Uuid::new_v4().to_string(),
                checksum: None,
            },
        );
    }

    pub async fn item_count(&self) -> usize {
        self.items.read().await.len()
    }

    /// Return the checksum stored for a path (for test assertions).
    ///
    /// Uses `try_read` — safe to call from sync test code after the upload future has resolved.
    pub fn get_checksum(&self, path: &RemotePath) -> Option<Checksum> {
        self.items
            .try_read()
            .ok()?
            .get(&path.0)
            .and_then(|i| i.checksum.clone())
    }

    /// Return the raw bytes stored at a path (for chunked-upload assertions).
    pub async fn get_data(&self, path: &RemotePath) -> Option<Vec<u8>> {
        self.items.read().await.get(&path.0).map(|i| i.data.clone())
    }

    /// Pre-seed a chunk for a given session (for resume tests).
    pub async fn seed_chunk(&self, session: &str, chunk_index: u32, data: &[u8]) {
        self.chunks
            .write()
            .await
            .insert((session.to_string(), chunk_index), data.to_vec());
    }
}

#[async_trait]
impl RemoteClient for MockRemoteClient {
    async fn list(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        let items = self.items.read().await;
        let prefix = path.0.trim_end_matches('/');
        let children: Vec<RemoteItem> = items
            .iter()
            .filter(|(k, _)| {
                let k = k.as_str();
                k.starts_with(prefix)
                    && k.len() > prefix.len()
                    && !k[prefix.len() + 1..].contains('/')
            })
            .map(|(k, v)| to_remote_item(k, v))
            .collect();
        Ok(children)
    }

    async fn list_recursive(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError> {
        let items = self.items.read().await;
        let prefix = path.0.trim_end_matches('/');
        let all: Vec<RemoteItem> = items
            .iter()
            .filter(|(k, _)| k.starts_with(prefix) && k.len() > prefix.len())
            .map(|(k, v)| to_remote_item(k, v))
            .collect();
        Ok(all)
    }

    async fn upload(
        &self,
        path: &RemotePath,
        data: ByteStream,
        _size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError> {
        self.upload_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        use futures::StreamExt;
        let mut bytes_vec = Vec::new();
        let mut stream = data;
        while let Some(chunk) = stream.next().await {
            bytes_vec.extend_from_slice(&chunk.map_err(|e| ClientError::Transient(e.to_string()))?);
        }
        let etag = format!("{:016x}", bytes_vec.len());
        let file_id = Uuid::new_v4().to_string();
        let mut items = self.items.write().await;
        items.insert(
            path.0.clone(),
            Item {
                data: bytes_vec,
                is_dir: false,
                etag: etag.clone(),
                file_id,
                checksum: Some(checksum.clone()),
            },
        );
        Ok(etag)
    }

    async fn begin_chunked_upload(&self) -> Result<String, ClientError> {
        self.begin_chunked_calls
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(format!("mock-session-{}", uuid::Uuid::new_v4()))
    }

    async fn upload_chunk(
        &self,
        session_url: &str,
        chunk_index: u32,
        data: ByteStream,
        _size: u64,
    ) -> Result<(), ClientError> {
        use futures::StreamExt;
        let mut bytes_vec = Vec::new();
        let mut stream = data;
        while let Some(chunk) = stream.next().await {
            bytes_vec.extend_from_slice(&chunk.map_err(|e| ClientError::Transient(e.to_string()))?);
        }
        self.chunks
            .write()
            .await
            .insert((session_url.to_string(), chunk_index), bytes_vec);
        Ok(())
    }

    async fn finalize_chunked_upload(
        &self,
        session_url: &str,
        dest: &RemotePath,
        _total_size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError> {
        // Assemble chunks in order.
        let chunks = self.chunks.read().await;
        let mut indices: Vec<u32> = chunks
            .keys()
            .filter(|(s, _)| s == session_url)
            .map(|(_, i)| *i)
            .collect();
        indices.sort_unstable();

        let mut assembled = Vec::new();
        for idx in indices {
            if let Some(data) = chunks.get(&(session_url.to_string(), idx)) {
                assembled.extend_from_slice(data);
            }
        }

        let etag = format!("{:016x}", assembled.len());
        let mut items = self.items.write().await;
        items.insert(
            dest.0.clone(),
            Item {
                data: assembled,
                is_dir: false,
                etag: etag.clone(),
                file_id: Uuid::new_v4().to_string(),
                checksum: Some(checksum.clone()),
            },
        );
        Ok(etag)
    }

    async fn list_uploaded_chunks(&self, session_url: &str) -> Result<Vec<u32>, ClientError> {
        let chunks = self.chunks.read().await;
        let mut indices: Vec<u32> = chunks
            .keys()
            .filter(|(s, _)| s == session_url)
            .map(|(_, i)| *i)
            .collect();
        indices.sort_unstable();
        Ok(indices)
    }

    async fn download(
        &self,
        path: &RemotePath,
        range: Option<ByteRange>,
    ) -> Result<ByteStream, ClientError> {
        let items = self.items.read().await;
        let item = items
            .get(&path.0)
            .ok_or_else(|| ClientError::Permanent(format!("not found: {}", path.0)))?;
        let data = match range {
            Some(r) => {
                let start = r.start as usize;
                let end = r.end.map(|e| e as usize + 1).unwrap_or(item.data.len());
                item.data[start..end].to_vec()
            }
            None => item.data.clone(),
        };
        let bytes = Bytes::from(data);
        let s: ByteStream = Box::pin(stream::once(async move { Ok::<_, std::io::Error>(bytes) }));
        Ok(s)
    }

    async fn delete(&self, path: &RemotePath) -> Result<(), ClientError> {
        let mut items = self.items.write().await;
        items.remove(&path.0);
        Ok(())
    }

    async fn move_item(&self, from: &RemotePath, to: &RemotePath) -> Result<(), ClientError> {
        let mut items = self.items.write().await;
        if let Some(item) = items.remove(&from.0) {
            items.insert(to.0.clone(), item);
        }
        Ok(())
    }

    async fn create_dir(&self, path: &RemotePath) -> Result<(), ClientError> {
        let mut items = self.items.write().await;
        items.insert(
            path.0.clone(),
            Item {
                data: vec![],
                is_dir: true,
                etag: "dir".into(),
                file_id: Uuid::new_v4().to_string(),
                checksum: None,
            },
        );
        Ok(())
    }

    async fn capabilities(&self) -> Result<ServerCapabilities, ClientError> {
        Ok(self.capabilities.clone())
    }
}

fn to_remote_item(path: &str, item: &Item) -> RemoteItem {
    use crate::types::RelativePath;
    RemoteItem {
        path: RelativePath::new(path),
        file_id: item.file_id.clone(),
        etag: item.etag.clone(),
        size: item.data.len() as u64,
        mtime: Utc::now(),
        checksum: None,
        is_dir: item.is_dir,
    }
}
