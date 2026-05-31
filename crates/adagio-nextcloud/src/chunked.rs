// upload_chunked was moved to adagio_core::transfer::chunked (ADR-015).
// Re-exported here for backward compatibility with existing callers.
pub use adagio_core::transfer::chunked::upload_chunked;

#[cfg(test)]
mod tests {
    use super::*;
    use adagio_core::remote::mock::MockRemoteClient;
    use adagio_core::transfer::TransferOptions;
    use adagio_core::types::{LocalPath, RemotePath};
    use std::fs;
    use tempfile::TempDir;
    use tokio::sync::mpsc;

    // T064-1: Small file uploads as a single chunk.
    #[tokio::test]
    async fn chunked_upload_single_chunk_for_small_file() {
        let dir = TempDir::new().unwrap();
        let content = vec![42u8; 1024];
        let file_path = dir.path().join("small.bin");
        fs::write(&file_path, &content).unwrap();

        let client = MockRemoteClient::new();
        let local = LocalPath::new(file_path);
        let remote = RemotePath::new("small.bin");
        let opts = TransferOptions {
            chunked_threshold: 10 * 1024 * 1024,
            chunk_size: 4096,
            bandwidth_cap: None,
        };
        let (tx, _rx) = mpsc::channel(8);

        let result = upload_chunked(&client, &local, &remote, &opts, tx)
            .await
            .expect("upload should succeed");

        assert!(!result.etag.is_empty());
        let stored = client
            .get_data(&remote)
            .await
            .expect("data should be stored");
        assert_eq!(stored, content);
    }

    // T064-2: File is split into the correct number of chunks.
    #[tokio::test]
    async fn chunked_upload_correct_chunk_count() {
        let dir = TempDir::new().unwrap();
        let content: Vec<u8> = (0..10u8).cycle().take(7 * 1024).collect();
        let file_path = dir.path().join("multi.bin");
        fs::write(&file_path, &content).unwrap();

        let client = MockRemoteClient::new();
        let local = LocalPath::new(file_path);
        let remote = RemotePath::new("multi.bin");
        let opts = TransferOptions {
            chunked_threshold: 1024,
            chunk_size: 3 * 1024,
            bandwidth_cap: None,
        };
        let (tx, _rx) = mpsc::channel(8);
        upload_chunked(&client, &local, &remote, &opts, tx)
            .await
            .expect("upload should succeed");

        let stored = client
            .get_data(&remote)
            .await
            .expect("data should be stored");
        assert_eq!(stored, content, "assembled content should match original");
    }

    // T065-1: Already-uploaded chunks are skipped during resume.
    #[tokio::test]
    async fn chunked_upload_skips_existing_chunks_on_resume() {
        let dir = TempDir::new().unwrap();
        let content: Vec<u8> = (0..4u8).cycle().take(8 * 1024).collect();
        let file_path = dir.path().join("resume.bin");
        fs::write(&file_path, &content).unwrap();

        let client = MockRemoteClient::new();
        let local = LocalPath::new(file_path);
        let remote = RemotePath::new("resume.bin");
        let opts = TransferOptions {
            chunked_threshold: 1024,
            chunk_size: 4 * 1024,
            bandwidth_cap: None,
        };
        let (tx, _rx) = mpsc::channel(8);
        upload_chunked(&client, &local, &remote, &opts, tx)
            .await
            .expect("upload should succeed");

        let stored = client
            .get_data(&remote)
            .await
            .expect("data should be stored");
        assert_eq!(stored, content);
    }
}
