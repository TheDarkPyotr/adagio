use crate::error::TransferError;
use crate::remote::RemoteClient;
use crate::transfer::{TransferOptions, TransferProgress, UploadResult};
use crate::types::{Checksum, ChecksumAlgorithm, LocalPath, RemotePath, TransferId};
use futures::stream;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;
use tracing::instrument;

/// Upload a large file using Nextcloud's chunked upload protocol.
///
/// - Creates a session directory on the server.
/// - Reads `opts.chunk_size` bytes at a time; skips chunks already present
///   to support resume.
/// - Finalises with a MOVE that assembles chunks into the destination path.
///
/// Callers are responsible for routing: only call this for files at or above
/// `opts.chunked_threshold`.
#[instrument(skip(client, progress), fields(remote_path = %remote_path))]
pub async fn upload_chunked(
    client: &dyn RemoteClient,
    local_path: &LocalPath,
    remote_path: &RemotePath,
    opts: &TransferOptions,
    progress: mpsc::Sender<TransferProgress>,
) -> Result<UploadResult, TransferError> {
    let path = local_path.0.clone();

    let (data, checksum) = tokio::task::spawn_blocking(move || read_and_hash(&path))
        .await
        .map_err(|e| TransferError::Transient(format!("join error: {e}")))?
        .map_err(|e| TransferError::Transient(e.to_string()))?;

    let total_size = data.len() as u64;
    let chunk_size = opts.chunk_size;

    let session_url = client
        .begin_chunked_upload()
        .await
        .map_err(|e| TransferError::Transient(e.to_string()))?;

    let uploaded_chunks: std::collections::HashSet<u32> = client
        .list_uploaded_chunks(&session_url)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    let num_chunks = total_size.div_ceil(chunk_size);
    let transfer_id = TransferId::new();
    let mut bytes_uploaded: u64 = 0;

    for chunk_index in 0..num_chunks as u32 {
        let start = (chunk_index as u64 * chunk_size) as usize;
        let end = ((chunk_index as u64 + 1) * chunk_size).min(total_size) as usize;
        let chunk_data = &data[start..end];
        let chunk_len = chunk_data.len() as u64;

        if uploaded_chunks.contains(&chunk_index) {
            bytes_uploaded += chunk_len;
            continue;
        }

        let bytes = bytes::Bytes::from(chunk_data.to_vec());
        let chunk_stream = Box::pin(stream::once(async move {
            Ok::<bytes::Bytes, std::io::Error>(bytes)
        }));

        client
            .upload_chunk(&session_url, chunk_index, chunk_stream, chunk_len)
            .await
            .map_err(|e| TransferError::Transient(e.to_string()))?;

        bytes_uploaded += chunk_len;
        let _ = progress.try_send(TransferProgress {
            transfer_id: transfer_id.clone(),
            bytes_transferred: bytes_uploaded,
            bytes_total: total_size,
            chunk_index: Some(chunk_index),
        });
    }

    let etag = client
        .finalize_chunked_upload(&session_url, remote_path, total_size, &checksum)
        .await
        .map_err(|e| TransferError::Transient(e.to_string()))?;

    Ok(UploadResult {
        etag,
        file_id: String::new(),
    })
}

fn read_and_hash(path: &std::path::Path) -> std::io::Result<(Vec<u8>, Checksum)> {
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
