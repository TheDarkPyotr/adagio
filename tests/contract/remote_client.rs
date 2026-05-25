use adagio_core::remote::mock::MockRemoteClient;
use adagio_core::remote::RemoteClient;
use adagio_core::types::{ByteRange, Checksum, ChecksumAlgorithm, RemotePath};
use bytes::Bytes;
use futures::stream;

fn checksum_for(data: &[u8]) -> Checksum {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    Checksum {
        algorithm: ChecksumAlgorithm::Sha256,
        value: hex::encode(hash),
    }
}

fn byte_stream(data: &'static [u8]) -> adagio_core::remote::ByteStream {
    Box::pin(stream::once(async move {
        Ok::<_, std::io::Error>(Bytes::from_static(data))
    }))
}

// ── Contract: upload makes item available via list_recursive ─────────────────

#[tokio::test]
async fn upload_then_list_recursive_shows_item() {
    let client = MockRemoteClient::new();
    let path = RemotePath::new("/remote/hello.txt");
    let data = b"hello world";
    let checksum = checksum_for(data);

    client
        .upload(&path, byte_stream(data), data.len() as u64, &checksum)
        .await
        .unwrap();

    let items = client
        .list_recursive(&RemotePath::new("/remote"))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path.as_str(), "remote/hello.txt");
    assert_eq!(items[0].size, data.len() as u64);
}

// ── Contract: download returns the same bytes as uploaded ────────────────────

#[tokio::test]
async fn upload_download_roundtrip() {
    use futures::StreamExt;

    let client = MockRemoteClient::new();
    let path = RemotePath::new("/remote/data.bin");
    let data: &[u8] = b"binary payload 12345";
    let checksum = checksum_for(data);

    client
        .upload(&path, byte_stream(data), data.len() as u64, &checksum)
        .await
        .unwrap();

    let mut stream = client.download(&path, None).await.unwrap();
    let mut received = Vec::new();
    while let Some(chunk) = stream.next().await {
        received.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(received, data);
}

// ── Contract: delete removes item from listing ───────────────────────────────

#[tokio::test]
async fn delete_removes_item_from_listing() {
    let client = MockRemoteClient::new();
    let path = RemotePath::new("/remote/to_delete.txt");
    let data = b"delete me";
    let checksum = checksum_for(data);

    client
        .upload(&path, byte_stream(data), data.len() as u64, &checksum)
        .await
        .unwrap();

    assert_eq!(client.item_count().await, 1);

    client.delete(&path).await.unwrap();

    let items = client
        .list_recursive(&RemotePath::new("/remote"))
        .await
        .unwrap();
    assert!(items.is_empty());
}

// ── Contract: move_item changes path in listing ──────────────────────────────

#[tokio::test]
async fn move_item_changes_path() {
    let client = MockRemoteClient::new();
    let from = RemotePath::new("/remote/old_name.txt");
    let to = RemotePath::new("/remote/new_name.txt");
    let data = b"rename me";
    let checksum = checksum_for(data);

    client
        .upload(&from, byte_stream(data), data.len() as u64, &checksum)
        .await
        .unwrap();

    client.move_item(&from, &to).await.unwrap();

    let items = client
        .list_recursive(&RemotePath::new("/remote"))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path.as_str(), "remote/new_name.txt");
}

// ── Contract: create_dir appears in listing ──────────────────────────────────

#[tokio::test]
async fn create_dir_appears_in_listing() {
    let client = MockRemoteClient::new();
    let dir = RemotePath::new("/remote/subdir");

    client.create_dir(&dir).await.unwrap();

    let items = client
        .list_recursive(&RemotePath::new("/remote"))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    assert!(items[0].is_dir);
}

// ── Contract: download with byte range returns partial data ──────────────────

#[tokio::test]
async fn download_range_returns_partial() {
    use futures::StreamExt;

    let client = MockRemoteClient::new();
    let path = RemotePath::new("/remote/partial.bin");
    let data = b"0123456789abcdef";
    let checksum = checksum_for(data);

    client
        .upload(&path, byte_stream(data), data.len() as u64, &checksum)
        .await
        .unwrap();

    let range = ByteRange {
        start: 4,
        end: Some(9),
    };
    let mut stream = client.download(&path, Some(range)).await.unwrap();
    let mut received = Vec::new();
    while let Some(chunk) = stream.next().await {
        received.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(received, b"456789");
}

// ── Contract: capabilities returns struct ────────────────────────────────────

#[tokio::test]
async fn capabilities_returns_server_info() {
    let client = MockRemoteClient::new();
    let caps = client.capabilities().await.unwrap();
    assert!(caps.max_chunk_size > 0);
}

// ── Contract: download non-existent path returns error ───────────────────────

#[tokio::test]
async fn download_missing_file_returns_permanent_error() {
    let client = MockRemoteClient::new();
    let result = client
        .download(&RemotePath::new("/remote/missing.txt"), None)
        .await;
    assert!(
        matches!(result, Err(adagio_core::error::ClientError::Permanent(_))),
        "expected Permanent error for missing file"
    );
}
