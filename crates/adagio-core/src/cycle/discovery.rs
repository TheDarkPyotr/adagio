use crate::error::SyncError;
use crate::remote::RemoteClient;
use crate::types::{LocalPath, RemotePath};

/// Summary presented to the user before starting the first sync.
#[derive(Debug, Clone)]
pub struct PreflightSummary {
    /// Number of items found on the remote.
    pub remote_item_count: usize,
    /// Total size of remote items in bytes.
    pub remote_total_bytes: u64,
    /// Number of directories found on the remote.
    pub remote_dir_count: usize,
    /// The local destination directory.
    pub local_root: LocalPath,
    /// The remote root that will be synced.
    pub remote_root: RemotePath,
}

/// Compute a pre-flight summary by listing the remote tree without downloading anything.
pub async fn preflight_summary(
    client: &dyn RemoteClient,
    remote_root: &RemotePath,
    local_root: LocalPath,
) -> Result<PreflightSummary, SyncError> {
    let items = client
        .list_recursive(remote_root)
        .await
        .map_err(SyncError::Remote)?;

    let remote_item_count = items.len();
    let remote_total_bytes: u64 = items.iter().filter(|i| !i.is_dir).map(|i| i.size).sum();
    let remote_dir_count = items.iter().filter(|i| i.is_dir).count();

    Ok(PreflightSummary {
        remote_item_count,
        remote_total_bytes,
        remote_dir_count,
        local_root,
        remote_root: remote_root.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::remote::mock::MockRemoteClient;
    use crate::types::LocalPath;

    #[tokio::test]
    async fn preflight_counts_remote_items() {
        let client = MockRemoteClient::new();
        client.seed("/remote/a.txt", b"hello").await;
        client.seed("/remote/b.txt", b"world").await;
        client.seed("/remote/sub/c.txt", b"!").await;

        let summary = preflight_summary(
            &client,
            &RemotePath::new("/remote"),
            LocalPath::new("/local/sync"),
        )
        .await
        .unwrap();

        assert_eq!(summary.remote_item_count, 3);
        assert_eq!(summary.remote_total_bytes, 5 + 5 + 1);
        assert_eq!(summary.remote_dir_count, 0);
    }
}
