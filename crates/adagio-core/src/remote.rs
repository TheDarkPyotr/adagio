use crate::error::ClientError;
use crate::types::{ByteRange, Checksum, RemoteItem, RemotePath, ServerCapabilities};
use async_trait::async_trait;
use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;

pub mod mock;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send>>;

/// HTTP-level abstraction over Nextcloud's WebDAV API.
///
/// `adagio-nextcloud` provides the production implementation.
/// Tests use an in-memory mock.
#[async_trait]
pub trait RemoteClient: Send + Sync {
    /// List the immediate children of a remote directory.
    async fn list(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError>;

    /// Recursively list all items under a remote directory.
    async fn list_recursive(&self, path: &RemotePath) -> Result<Vec<RemoteItem>, ClientError>;

    /// Upload a file using a single PUT request (for files below chunked threshold).
    ///
    /// `checksum` is sent in the `OC-Checksum` header so the server verifies integrity.
    async fn upload(
        &self,
        path: &RemotePath,
        data: ByteStream,
        size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError>;

    /// Begin a Nextcloud chunked upload session.
    ///
    /// Returns the upload session URL.
    async fn begin_chunked_upload(&self) -> Result<String, ClientError>;

    /// Upload a single chunk to an active chunked upload session.
    async fn upload_chunk(
        &self,
        session_url: &str,
        chunk_index: u32,
        data: ByteStream,
        size: u64,
    ) -> Result<(), ClientError>;

    /// Finalize a chunked upload by MOVEing the temp assembly to the destination.
    async fn finalize_chunked_upload(
        &self,
        session_url: &str,
        dest: &RemotePath,
        total_size: u64,
        checksum: &Checksum,
    ) -> Result<String, ClientError>;

    /// List which chunk indices have already been uploaded (for resume).
    async fn list_uploaded_chunks(&self, session_url: &str) -> Result<Vec<u32>, ClientError>;

    /// Download a remote file, optionally resuming from a byte offset.
    async fn download(
        &self,
        path: &RemotePath,
        range: Option<ByteRange>,
    ) -> Result<ByteStream, ClientError>;

    /// Delete a remote item (file or empty directory).
    async fn delete(&self, path: &RemotePath) -> Result<(), ClientError>;

    /// Move/rename a remote item atomically.
    async fn move_item(&self, from: &RemotePath, to: &RemotePath) -> Result<(), ClientError>;

    /// Create a remote directory (MKCOL), including all parents.
    async fn create_dir(&self, path: &RemotePath) -> Result<(), ClientError>;

    /// Fetch server capabilities (chunked upload support, max sizes, etc.).
    async fn capabilities(&self) -> Result<ServerCapabilities, ClientError>;
}
