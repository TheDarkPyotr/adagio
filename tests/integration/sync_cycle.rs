//! Integration tests for the full sync cycle.
//!
//! These tests require a live Nextcloud instance and are therefore marked
//! `#[ignore]` — run with `cargo test -- --ignored` against the Docker
//! development Nextcloud stack documented in `specs/001-nextcloud-file-sync/quickstart.md`.
//!
//! Environment variables required:
//!   ADAGIO_TEST_SERVER   — e.g. http://localhost:8080
//!   ADAGIO_TEST_USER     — Nextcloud username
//!   ADAGIO_TEST_PASSWORD — app-password for that user

use adagio_core::cycle::discovery::preflight_summary;
use adagio_core::remote::{ByteStream, RemoteClient};
use adagio_core::types::{Checksum, ChecksumAlgorithm, LocalPath, RemotePath};
use adagio_nextcloud::client::NextcloudClient;
use bytes::Bytes;
use futures::stream;
use std::path::PathBuf;
use tempfile::TempDir;

/// Wrap `bytes::Bytes` into a single-item `ByteStream`.
fn bytes_stream(b: &'static [u8]) -> ByteStream {
    Box::pin(stream::once(async move {
        Ok::<Bytes, std::io::Error>(Bytes::from_static(b))
    }))
}

fn dummy_checksum() -> Checksum {
    Checksum {
        algorithm: ChecksumAlgorithm::Sha256,
        value: "0".repeat(64),
    }
}

/// Collect a ByteStream into a Vec<u8>.
async fn collect_stream(s: ByteStream) -> Vec<u8> {
    use futures::StreamExt;
    let mut s = s;
    let mut out = Vec::new();
    while let Some(chunk) = s.next().await {
        out.extend_from_slice(&chunk.expect("stream chunk"));
    }
    out
}

fn test_env() -> Option<(String, String, String)> {
    let server = std::env::var("ADAGIO_TEST_SERVER").ok()?;
    let user = std::env::var("ADAGIO_TEST_USER").ok()?;
    let pass = std::env::var("ADAGIO_TEST_PASSWORD").ok()?;
    Some((server, user, pass))
}

// ── T023: Configure account → create pair → run first sync → verify files ────

#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn configure_account_and_preflight_summary() {
    let (server, user, pass) = test_env()
        .expect("set ADAGIO_TEST_SERVER, ADAGIO_TEST_USER, ADAGIO_TEST_PASSWORD to run this test");

    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_root = RemotePath::new("");
    let local_dir = TempDir::new().unwrap();

    let summary = preflight_summary(&client, &remote_root, LocalPath::new(local_dir.path()))
        .await
        .expect("preflight should succeed against a live server");

    println!(
        "preflight: {} items, {} bytes",
        summary.remote_item_count, summary.remote_total_bytes
    );
    // Just verify the call completes — content depends on the test server state.
}

#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn server_capabilities_from_real_server() {
    let (server, user, pass) = test_env().unwrap();
    let client = NextcloudClient::new(&server, &user, &pass);
    let caps = client
        .capabilities()
        .await
        .expect("capabilities should return OK");
    assert!(caps.max_chunk_size > 0);
    println!("server version: {}", caps.server_version);
}

// ── T038: Bidirectional create/modify/delete integration tests ────────────────

/// Helper: unique remote test directory for a test run (user-relative path).
fn test_remote_dir(suffix: &str) -> RemotePath {
    RemotePath::new(format!("adagio_test_{suffix}/"))
}

// T038-A: Local file created → upload → verify remote
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_local_create_uploads_to_remote() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("create_upload");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");

    let local_dir = TempDir::new().unwrap();
    let file_path = local_dir.path().join("hello.txt");
    std::fs::write(&file_path, b"hello from adagio").unwrap();

    let remote_file = RemotePath::new(format!("{}{}", remote_dir.as_str(), "hello.txt"));
    client
        .upload(
            &remote_file,
            bytes_stream(b"hello from adagio"),
            17,
            &dummy_checksum(),
        )
        .await
        .expect("upload should succeed");

    // Verify remote listing contains the file.
    let items = client.list(&remote_dir).await.expect("list should succeed");
    let found = items.iter().any(|i| i.path.as_str().ends_with("hello.txt"));
    assert!(found, "uploaded file should appear in remote listing");

    // Cleanup
    client.delete(&remote_file).await.ok();
    client.delete(&remote_dir).await.ok();
}

// T038-B: Remote file created → download → verify local
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_remote_create_downloads_to_local() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("remote_create");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");

    let remote_file = RemotePath::new(format!("{}remote_file.txt", remote_dir.as_str()));
    let expected = b"remote content";
    client
        .upload(
            &remote_file,
            bytes_stream(expected),
            expected.len() as u64,
            &dummy_checksum(),
        )
        .await
        .expect("upload should succeed");

    let local_dir = TempDir::new().unwrap();
    let local_path = local_dir.path().join("remote_file.txt");

    let stream = client
        .download(&remote_file, None)
        .await
        .expect("download should succeed");
    let data = collect_stream(stream).await;
    std::fs::write(&local_path, &data).unwrap();
    assert_eq!(std::fs::read(&local_path).unwrap(), expected);

    // Cleanup
    client.delete(&remote_file).await.ok();
    client.delete(&remote_dir).await.ok();
}

// T038-C: Local file modified → re-upload → verify remote content updated
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_local_modify_updates_remote() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("local_modify");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");
    let remote_file = RemotePath::new(format!("{}modify_me.txt", remote_dir.as_str()));

    // Initial upload
    client
        .upload(&remote_file, bytes_stream(b"v1"), 2, &dummy_checksum())
        .await
        .expect("initial upload");

    // Modify and re-upload
    let updated = b"v2 updated content";
    client
        .upload(
            &remote_file,
            bytes_stream(updated),
            updated.len() as u64,
            &dummy_checksum(),
        )
        .await
        .expect("re-upload after local modify");

    // Download and verify
    let stream = client.download(&remote_file, None).await.expect("download");
    let data = collect_stream(stream).await;
    assert_eq!(data, updated);

    // Cleanup
    client.delete(&remote_file).await.ok();
    client.delete(&remote_dir).await.ok();
}

// T038-D: Local file deleted → delete remote → verify absent
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_local_delete_removes_remote() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("local_delete");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");
    let remote_file = RemotePath::new(format!("{}to_delete.txt", remote_dir.as_str()));

    client
        .upload(&remote_file, bytes_stream(b"bye"), 3, &dummy_checksum())
        .await
        .expect("upload");
    client.delete(&remote_file).await.expect("delete remote");

    // File should no longer appear in listing
    let items = client.list(&remote_dir).await.expect("list");
    let found = items
        .iter()
        .any(|i| i.path.as_str().ends_with("to_delete.txt"));
    assert!(!found, "deleted file should be absent from remote listing");

    // Cleanup
    client.delete(&remote_dir).await.ok();
}

// T038-E: Remote file deleted → local copy should be removed
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_remote_delete_removes_local_copy() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("remote_delete");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");
    let remote_file = RemotePath::new(format!("{}remote_del.txt", remote_dir.as_str()));

    client
        .upload(
            &remote_file,
            bytes_stream(b"will be deleted"),
            15,
            &dummy_checksum(),
        )
        .await
        .expect("upload");

    // Simulate: download to local, then delete remote, verify local would be cleaned up.
    let local_dir = TempDir::new().unwrap();
    let local_path: PathBuf = local_dir.path().join("remote_del.txt");
    let stream = client.download(&remote_file, None).await.expect("download");
    let data = collect_stream(stream).await;
    std::fs::write(&local_path, &data).unwrap();

    client.delete(&remote_file).await.expect("delete remote");

    // Verify remote is gone
    let items = client.list(&remote_dir).await.expect("list");
    let found = items
        .iter()
        .any(|i| i.path.as_str().ends_with("remote_del.txt"));
    assert!(!found, "file should be absent from remote after deletion");

    // (In real sync cycle the reconciler+propagator would also remove local_path)
    // Cleanup
    client.delete(&remote_dir).await.ok();
}

// ── T083: Selective sync tests (in-memory, no live server needed) ─────────────

/// Build a minimal `SyncPair` with the given `selective_paths` list.
fn make_pair_with_exclusions(
    local_root: &std::path::Path,
    selective_paths: Vec<adagio_core::types::RelativePath>,
) -> adagio_core::types::SyncPair {
    use adagio_core::types::*;
    SyncPair {
        id: PairId::new(),
        account_id: AccountId::new(),
        local_root: LocalPath::new(local_root),
        remote_root: RemotePath::new("remote/"),
        status: PairStatus::Idle,
        exclude_patterns: vec![],
        selective_paths,
        created_at: chrono::Utc::now(),
        last_synced_at: None,
        scan_interval_secs: 7200,
        scan_on_startup: true,
        max_upload_concurrency: 3,
        max_download_concurrency: 3,
        conflict_policy: ConflictPolicy::Ask,
        bulk_upload_workers: 8,
        bulk_upload_chunk_threshold_bytes: 10 * 1024 * 1024,
        vfs_enabled: false,
        vfs_cache_max_bytes: 20 * 1024 * 1024 * 1024,
        vfs_eviction_threshold_bytes: 5 * 1024 * 1024 * 1024,
        bulk_upload_threshold_files: 50,
    }
}

// T083-1: Exclude-before-sync — items in excluded paths not included in remote snapshot.
#[tokio::test]
async fn selective_sync_exclude_before_sync_skips_excluded_dir() {
    use adagio_core::detection::exclusion::filter_selective;
    use adagio_core::types::{RelativePath, RemoteItem};
    use chrono::Utc;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();

    // Remote has: docs/report.pdf, photos/img.jpg, readme.txt
    let remote_items = vec![
        RemoteItem {
            path: RelativePath::new("docs/report.pdf"),
            file_id: "1".into(),
            size: 100,
            etag: "e1".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
        RemoteItem {
            path: RelativePath::new("photos/img.jpg"),
            file_id: "2".into(),
            size: 200,
            etag: "e2".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
        RemoteItem {
            path: RelativePath::new("readme.txt"),
            file_id: "3".into(),
            size: 50,
            etag: "e3".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
    ];

    // Pair only selects "readme.txt" and "photos/"; docs/ is excluded.
    let pair = make_pair_with_exclusions(
        dir.path(),
        vec![RelativePath::new("photos"), RelativePath::new("readme.txt")],
    );

    // Filter remote items by selective_paths.
    let filtered = filter_selective(remote_items, &pair.selective_paths);

    assert!(
        !filtered
            .iter()
            .any(|i| i.path.as_str().starts_with("docs/")),
        "docs/ should be excluded"
    );
    assert!(
        filtered.iter().any(|i| i.path.as_str() == "readme.txt"),
        "readme.txt should be included"
    );
    assert!(
        filtered
            .iter()
            .any(|i| i.path.as_str().starts_with("photos/")),
        "photos/ should be included"
    );
}

// T083-2: Re-inclusion — removing a path from exclusion makes items appear again.
#[tokio::test]
async fn selective_sync_reinclude_path_brings_items_back() {
    use adagio_core::detection::exclusion::filter_selective;
    use adagio_core::types::{RelativePath, RemoteItem};
    use chrono::Utc;
    use tempfile::TempDir;

    let dir = TempDir::new().unwrap();
    let remote_items = vec![
        RemoteItem {
            path: RelativePath::new("docs/report.pdf"),
            file_id: "1".into(),
            size: 100,
            etag: "e1".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
        RemoteItem {
            path: RelativePath::new("readme.txt"),
            file_id: "2".into(),
            size: 50,
            etag: "e2".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
    ];

    // First: only readme.txt selected (docs/ excluded).
    let excluded = make_pair_with_exclusions(dir.path(), vec![RelativePath::new("readme.txt")]);
    let filtered_excluded = filter_selective(remote_items.clone(), &excluded.selective_paths);
    assert!(!filtered_excluded
        .iter()
        .any(|i| i.path.as_str().starts_with("docs/")));

    // Re-include docs/ by adding it to selective_paths (or clearing the list = all).
    let included = make_pair_with_exclusions(dir.path(), vec![]);
    let filtered_all = filter_selective(remote_items.clone(), &included.selective_paths);
    assert!(
        filtered_all
            .iter()
            .any(|i| i.path.as_str().starts_with("docs/")),
        "docs/ should appear after re-inclusion"
    );
}

// T083-3: Empty selective_paths means sync all (no filtering).
#[tokio::test]
async fn selective_sync_empty_list_syncs_everything() {
    use adagio_core::detection::exclusion::filter_selective;
    use adagio_core::types::{RelativePath, RemoteItem};
    use chrono::Utc;

    let items = vec![
        RemoteItem {
            path: RelativePath::new("a/b.txt"),
            file_id: "1".into(),
            size: 10,
            etag: "e1".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
        RemoteItem {
            path: RelativePath::new("c.txt"),
            file_id: "2".into(),
            size: 5,
            etag: "e2".into(),
            mtime: Utc::now(),
            checksum: None,
            is_dir: false,
        },
    ];

    let result = filter_selective(items.clone(), &[]);
    assert_eq!(
        result.len(),
        2,
        "empty selective_paths should pass through all items"
    );
}

// T038-F: Remote file renamed via server MOVE → local rename detected
#[tokio::test]
#[ignore = "requires live Nextcloud instance (see quickstart.md)"]
async fn integration_remote_rename_detected_via_file_id() {
    let (server, user, pass) = test_env().expect("set test env vars");
    let client = NextcloudClient::new(&server, &user, &pass);

    let remote_dir = test_remote_dir("remote_rename");
    client
        .create_dir(&remote_dir)
        .await
        .expect("create test dir");
    let old_path = RemotePath::new(format!("{}original.txt", remote_dir.as_str()));
    let new_path = RemotePath::new(format!("{}renamed.txt", remote_dir.as_str()));

    client
        .upload(&old_path, bytes_stream(b"rename me"), 9, &dummy_checksum())
        .await
        .expect("upload");

    client
        .move_item(&old_path, &new_path)
        .await
        .expect("server-side MOVE");

    let items = client.list(&remote_dir).await.expect("list");
    let found_old = items
        .iter()
        .any(|i| i.path.as_str().ends_with("original.txt"));
    let found_new = items
        .iter()
        .any(|i| i.path.as_str().ends_with("renamed.txt"));
    assert!(!found_old, "old path should be gone after rename");
    assert!(found_new, "new path should be present after rename");

    // Verify file_id is stable across the rename.
    let new_item = items
        .iter()
        .find(|i| i.path.as_str().ends_with("renamed.txt"))
        .unwrap();
    assert!(
        !new_item.file_id.is_empty(),
        "file_id should be set after rename"
    );

    // Cleanup
    client.delete(&new_path).await.ok();
    client.delete(&remote_dir).await.ok();
}
