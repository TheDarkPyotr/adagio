# Contract: RemoteClient

**Crate**: `adagio-core` (trait definition) / `adagio-nextcloud` (implementation)
**Module**: `adagio_core::cycle::discovery`, `adagio_core::transfer`

`RemoteClient` abstracts all communication with the Nextcloud server. The sync engine
depends only on this trait; the Nextcloud-specific WebDAV + chunked-upload implementation
lives in `adagio-nextcloud`. This enables testing the engine against a mock client.

## Trait Definition

```rust
/// Abstracts all remote server operations.
#[async_trait]
pub trait RemoteClient: Send + Sync {
    // ── Discovery ────────────────────────────────────────────────────────────

    /// List items in a directory (non-recursive).
    ///
    /// Returns one `RemoteItem` per child. The directory itself is NOT included.
    async fn list(
        &self,
        path: &RemotePath,
    ) -> Result<Vec<RemoteItem>, ClientError>;

    /// List an entire subtree recursively.
    ///
    /// Implementations SHOULD use a server-side recursive PROPFIND (Depth: infinity)
    /// when the server supports it, falling back to recursive manual traversal.
    async fn list_recursive(
        &self,
        root: &RemotePath,
    ) -> Result<Vec<RemoteItem>, ClientError>;

    // ── Transfers ─────────────────────────────────────────────────────────────

    /// Upload a file as a single PUT.
    ///
    /// `content` is a streaming body. The caller MUST NOT buffer the entire file.
    /// `checksum` is sent as the OC-Checksum header.
    async fn upload(
        &self,
        path: &RemotePath,
        content: impl AsyncRead + Send + Unpin + 'static,
        size: u64,
        checksum: Option<Checksum>,
    ) -> Result<UploadResult, ClientError>;

    /// Begin a chunked upload session, returning a session ID.
    async fn begin_chunked_upload(
        &self,
        path: &RemotePath,
        total_size: u64,
    ) -> Result<ChunkedUploadSession, ClientError>;

    /// Upload one chunk of an existing session.
    async fn upload_chunk(
        &self,
        session: &ChunkedUploadSession,
        chunk_index: u32,
        data: impl AsyncRead + Send + Unpin + 'static,
        chunk_size: u64,
    ) -> Result<(), ClientError>;

    /// Finalize a chunked upload session by issuing a MOVE.
    async fn finalize_chunked_upload(
        &self,
        session: &ChunkedUploadSession,
        checksum: Option<Checksum>,
    ) -> Result<UploadResult, ClientError>;

    /// List chunks already present in an upload session (for resume).
    async fn list_uploaded_chunks(
        &self,
        session: &ChunkedUploadSession,
    ) -> Result<Vec<u32>, ClientError>;

    /// Download a file, returning a streaming reader.
    async fn download(
        &self,
        path: &RemotePath,
        range: Option<ByteRange>,
    ) -> Result<impl AsyncRead + Send + Unpin, ClientError>;

    // ── Mutations ─────────────────────────────────────────────────────────────

    /// Delete an item (file or empty directory).
    async fn delete(&self, path: &RemotePath) -> Result<(), ClientError>;

    /// Move/rename an item server-side.
    async fn move_item(
        &self,
        from: &RemotePath,
        to: &RemotePath,
    ) -> Result<(), ClientError>;

    /// Create a directory.
    async fn create_dir(&self, path: &RemotePath) -> Result<(), ClientError>;

    // ── Metadata ──────────────────────────────────────────────────────────────

    /// Fetch server capabilities (max upload size, checksum algorithms, etc.).
    async fn capabilities(&self) -> Result<ServerCapabilities, ClientError>;
}
```

## Associated Types

```rust
pub struct RemoteItem {
    pub relative_path: RelativePath,
    pub item_type: ItemType,
    pub size_bytes: Option<u64>,
    pub etag: String,
    pub file_id: Option<String>,
    pub last_modified: DateTime<Utc>,
}

pub struct UploadResult {
    pub etag: String,
    pub file_id: Option<String>,
}

pub struct ChunkedUploadSession {
    pub session_id: String,
    pub destination_path: RemotePath,
    pub chunk_size: u64,
}

pub struct ServerCapabilities {
    pub max_upload_size: u64,
    pub chunked_upload_threshold: u64,
    pub preferred_checksum: ChecksumAlgorithm,
    pub supported_checksums: Vec<ChecksumAlgorithm>,
}

pub struct ByteRange {
    pub start: u64,
    pub end: Option<u64>,
}
```

## Error Classification

```rust
pub enum ClientError {
    /// Retry-eligible: 5xx, 429, timeout, network drop.
    Transient(TransientClientError),
    /// Do not retry: 4xx (except 401/408/423/429), invalid path, server mismatch.
    Permanent(PermanentClientError),
    /// Authentication failure: pause pair, surface re-auth prompt.
    AuthRequired,
}
```

## Contract Guarantees

1. `download` returns a reader positioned at `range.start` when a range is given;
   the caller is responsible for verifying the checksum of the full content after
   appending range responses.
2. `list_recursive` returns items in breadth-first order; callers MUST NOT assume order.
3. `delete` on a non-existent path returns `Ok(())` (idempotent).
4. All methods are cancellation-safe: dropping a future mid-operation does not leave
   orphaned server state (except `begin_chunked_upload` which creates a session the
   caller must eventually finalize or abandon).
