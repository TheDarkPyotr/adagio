use crate::error::TransferError;
use crate::types::{Checksum, LocalPath, RemotePath, SyncPair, TransferId};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

pub mod download;
pub mod engine;
pub mod upload;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferOptions {
    /// Threshold above which chunked upload is used.
    /// Defaults to min(server_max_upload, 10 MB).
    pub chunked_threshold: u64,
    /// Size of each chunk for chunked uploads. Default: 10 MB.
    pub chunk_size: u64,
    /// Optional bandwidth cap (bytes/sec). None = unlimited.
    pub bandwidth_cap: Option<u64>,
}

impl Default for TransferOptions {
    fn default() -> Self {
        const TEN_MB: u64 = 10 * 1024 * 1024;
        Self {
            chunked_threshold: TEN_MB,
            chunk_size: TEN_MB,
            bandwidth_cap: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferProgress {
    pub transfer_id: TransferId,
    pub bytes_transferred: u64,
    pub bytes_total: u64,
    pub chunk_index: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadResult {
    pub etag: String,
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadResult {
    /// Checksum of the downloaded content (verified against expected).
    pub checksum: Checksum,
    /// Final size in bytes.
    pub size: u64,
}

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
    ///
    /// Safe to call on a transfer that has already completed; returns `Ok(())`.
    async fn cancel(&self, transfer_id: TransferId) -> Result<(), TransferError>;
}
