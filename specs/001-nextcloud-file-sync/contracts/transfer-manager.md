# Contract: TransferManager

**Crate**: `adagio-core`
**Module**: `crate::transfer`

`TransferManager` handles file upload and download with streaming I/O, checksum
verification, chunked upload for large files, range-request resume for downloads,
and bandwidth throttling.

## Trait Definition

```rust
/// Executes individual file transfers with integrity verification and resume support.
#[async_trait]
pub trait TransferManager: Send + Sync {
    /// Upload a local file to the remote.
    ///
    /// - Files below `opts.chunked_threshold` are uploaded as a single PUT.
    /// - Files at or above the threshold use chunked upload protocol.
    /// - Content is streamed from disk; the file MUST NOT be fully loaded into memory.
    /// - Checksum is verified against the server's acknowledgment on completion.
    ///
    /// Returns the remote etag and file ID of the newly uploaded file.
    async fn upload(
        &self,
        pair: &SyncPair,
        local_path: &LocalPath,
        remote_path: &RemotePath,
        opts: TransferOptions,
        progress: mpsc::Sender<TransferProgress>,
    ) -> Result<UploadResult, TransferError>;

    /// Download a remote file to a local path.
    ///
    /// Downloads stream to a `.adagio_tmp_<uuid>` file in the same directory as
    /// the final destination. On successful completion and checksum verification,
    /// the temp file is atomically renamed to `local_path`.
    ///
    /// If an interrupted download exists (temp file present with expected size),
    /// the download resumes using a range request.
    async fn download(
        &self,
        pair: &SyncPair,
        remote_path: &RemotePath,
        local_path: &LocalPath,
        expected_checksum: Option<Checksum>,
        progress: mpsc::Sender<TransferProgress>,
    ) -> Result<DownloadResult, TransferError>;

    /// Cancel an in-progress transfer.
    ///
    /// The current chunk (if any) is allowed to complete. The transfer is
    /// marked as interrupted; the temp file and any chunked upload session
    /// are preserved for future resumption.
    async fn cancel(&self, transfer_id: TransferId) -> Result<(), TransferError>;
}
```

## Associated Types

```rust
pub struct TransferOptions {
    /// Threshold above which chunked upload is used. Defaults to
    /// min(server_max_upload, 10 MB).
    pub chunked_threshold: u64,
    /// Size of each chunk for chunked uploads. Default: 10 MB.
    pub chunk_size: u64,
    /// Optional bandwidth cap (bytes/sec). None = unlimited.
    pub bandwidth_cap: Option<u64>,
}

pub struct TransferProgress {
    pub transfer_id: TransferId,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub chunk_index: Option<u32>,
}

pub struct DownloadResult {
    /// Checksum of the downloaded content (verified against expected).
    pub checksum: Checksum,
    /// Final size in bytes.
    pub size: u64,
}
```

## Error Classification

```rust
pub enum TransferError {
    /// Retry-eligible: network, server 5xx, timeout.
    Transient(String),
    /// Do not retry: checksum mismatch (repeated), disk full, permission denied.
    Permanent(String),
    /// Caller should detect file-in-progress and defer to next cycle.
    FileInProgress,
}
```

## Contract Guarantees

1. Downloads NEVER appear at the final destination path until the checksum is verified
   and the rename completes. A disk-full failure leaves the existing file at
   `local_path` intact.
2. Upload checksum verification is end-to-end: the SHA-256 (or server-preferred
   algorithm) is computed incrementally during streaming and matched against the server's
   `OC-Checksum` response header.
3. `cancel` is safe to call on a transfer that has already completed; returns `Ok(())`.
4. Progress events are emitted at least once per chunk (or per 1 MB for single-PUT
   transfers).
5. `TransferError::FileInProgress` is NOT an error to surface to the user; the engine
   MUST defer the item to the next sync cycle.
