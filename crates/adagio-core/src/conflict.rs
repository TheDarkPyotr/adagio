use crate::error::{JournalError, TransferError};
use crate::journal::Journal;
use crate::remote::RemoteClient;
use crate::transfer::download::download_file;
use crate::transfer::upload::upload_single;
use crate::transfer::TransferOptions;
use crate::types::{
    ConflictPolicy, ConflictRecord, ConflictResolution, ConflictSide, LocalPath, PairId,
    RelativePath, RemotePath,
};
use chrono::{DateTime, Utc};
use std::path::Path;
use tokio::sync::mpsc;
use tracing::instrument;

// ── Conflict copy naming (T057) ───────────────────────────────────────────────

/// Build the conflict-copy path for a file.
///
/// Format: `<stem> (conflicted copy from <device> YYYY-MM-DD HH-MM-SS)<ext>`
///
/// Example: `report (conflicted copy from MacBook 2024-03-15 14-32-07).docx`
pub fn conflict_copy_path(
    original: &RelativePath,
    device_name: &str,
    at: DateTime<Utc>,
) -> RelativePath {
    let p = std::path::Path::new(original.as_str());
    let stem = p.file_stem().unwrap_or_default().to_string_lossy();
    let ext = p
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let parent = p
        .parent()
        .filter(|p| p != &Path::new(""))
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default();
    let timestamp = at.format("%Y-%m-%d %H-%M-%S").to_string();
    let name = format!("{stem} (conflicted copy from {device_name} {timestamp}){ext}");
    RelativePath::new(format!("{parent}{name}"))
}

// ── Conflict resolver (T056) ──────────────────────────────────────────────────

/// Outcome produced by `resolve`.
#[derive(Debug)]
pub struct ResolveOutcome {
    pub resolution: ConflictResolution,
}

/// Resolve a conflict according to the configured policy.
///
/// Returns the `ConflictResolution` to be persisted in the journal.
#[allow(clippy::too_many_arguments)]
#[instrument(skip(client, _journal, progress_tx), fields(path = %record.path, policy = ?record.policy))]
pub async fn resolve(
    record: &ConflictRecord,
    pair_id: &PairId,
    local_root: &LocalPath,
    remote_root: &RemotePath,
    client: &dyn RemoteClient,
    _journal: &dyn Journal,
    device_name: &str,
    user_choice: Option<ConflictSide>, // populated only for Ask policy
    progress_tx: mpsc::Sender<crate::transfer::TransferProgress>,
) -> Result<ResolveOutcome, TransferError> {
    let opts = TransferOptions::default();
    let local_file = LocalPath::new(local_root.0.join(record.path.as_str()));
    let remote_file = RemotePath::new(format!(
        "{}/{}",
        remote_root.as_str().trim_end_matches('/'),
        record.path.as_str()
    ));

    let resolution = match &record.policy {
        // ── PreserveBoth ──────────────────────────────────────────────────────
        // 1. Download remote → its current local path (overwrites local).
        // 2. Name the original local version as a conflict copy.
        // 3. Upload the conflict copy to remote.
        ConflictPolicy::PreserveBoth => {
            let copy_path = conflict_copy_path(&record.path, device_name, Utc::now());
            let local_copy = LocalPath::new(local_root.0.join(copy_path.as_str()));

            // Rename existing local file to conflict-copy path before download overwrites it.
            if local_file.0.exists() {
                if let Some(parent) = local_copy.0.parent() {
                    tokio::fs::create_dir_all(parent)
                        .await
                        .map_err(|e| TransferError::Transient(e.to_string()))?;
                }
                tokio::fs::rename(&local_file.0, &local_copy.0)
                    .await
                    .map_err(|e| TransferError::Transient(e.to_string()))?;
            }

            // Download the remote version to the canonical local path.
            download_file(
                client,
                &remote_file,
                &local_file,
                None,
                &opts,
                progress_tx.clone(),
            )
            .await?;

            // Upload the conflict copy to remote.
            let remote_copy = RemotePath::new(format!(
                "{}/{}",
                remote_root.as_str().trim_end_matches('/'),
                copy_path.as_str()
            ));
            upload_single(client, &local_copy, &remote_copy, &opts, progress_tx).await?;

            ConflictResolution::BothKept {
                conflict_copy_path: copy_path,
            }
        }

        // ── LocalWins ─────────────────────────────────────────────────────────
        ConflictPolicy::LocalWins => {
            upload_single(client, &local_file, &remote_file, &opts, progress_tx).await?;
            ConflictResolution::KeptLocal
        }

        // ── RemoteWins ────────────────────────────────────────────────────────
        ConflictPolicy::RemoteWins => {
            download_file(client, &remote_file, &local_file, None, &opts, progress_tx).await?;
            ConflictResolution::KeptRemote
        }

        // ── NewestWins ────────────────────────────────────────────────────────
        ConflictPolicy::NewestWins => {
            if record.local_mtime >= record.remote_mtime {
                upload_single(client, &local_file, &remote_file, &opts, progress_tx).await?;
                ConflictResolution::KeptLocal
            } else {
                download_file(client, &remote_file, &local_file, None, &opts, progress_tx).await?;
                ConflictResolution::KeptRemote
            }
        }

        // ── Ask ───────────────────────────────────────────────────────────────
        ConflictPolicy::Ask => match user_choice {
            Some(ConflictSide::Local) => {
                upload_single(client, &local_file, &remote_file, &opts, progress_tx).await?;
                ConflictResolution::KeptLocal
            }
            Some(ConflictSide::Remote) => {
                download_file(client, &remote_file, &local_file, None, &opts, progress_tx).await?;
                ConflictResolution::KeptRemote
            }
            None => {
                return Err(TransferError::Permanent(
                    "Ask policy requires a user_choice".into(),
                ));
            }
        },
    };

    Ok(ResolveOutcome { resolution })
}

// ── Conflict record persistence (T058) ───────────────────────────────────────

/// Persist a new conflict record via the journal store.
pub async fn record_conflict(
    journal: &dyn Journal,
    record: &ConflictRecord,
) -> Result<(), JournalError> {
    journal.upsert_conflict(record).await
}

/// Mark a conflict record as resolved.
pub async fn mark_resolved(
    journal: &dyn Journal,
    id: &str,
    resolution: ConflictResolution,
) -> Result<(), JournalError> {
    journal.resolve_conflict(id, resolution).await
}

// ── Tests (T053) ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RelativePath;
    use chrono::TimeZone;
    use uuid::Uuid;

    fn ts(y: i32, mo: u32, d: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, m, s).unwrap()
    }

    // T053-1: Flat file — correct stem, suffix, extension.
    #[test]
    fn conflict_copy_flat_file() {
        let p = RelativePath::new("report.docx");
        let at = ts(2024, 3, 15, 14, 32, 7);
        let copy = conflict_copy_path(&p, "MacBook", at);
        assert_eq!(
            copy.as_str(),
            "report (conflicted copy from MacBook 2024-03-15 14-32-07).docx"
        );
    }

    // T053-2: File with no extension.
    #[test]
    fn conflict_copy_no_extension() {
        let p = RelativePath::new("Makefile");
        let at = ts(2024, 1, 1, 0, 0, 0);
        let copy = conflict_copy_path(&p, "Desktop", at);
        assert_eq!(
            copy.as_str(),
            "Makefile (conflicted copy from Desktop 2024-01-01 00-00-00)"
        );
    }

    // T053-3: File in subdirectory — parent path is preserved.
    #[test]
    fn conflict_copy_preserves_parent_dir() {
        let p = RelativePath::new("docs/notes.md");
        let at = ts(2024, 6, 10, 9, 5, 3);
        let copy = conflict_copy_path(&p, "Laptop", at);
        assert_eq!(
            copy.as_str(),
            "docs/notes (conflicted copy from Laptop 2024-06-10 09-05-03).md"
        );
    }

    // T053-4: Device name with spaces works correctly.
    #[test]
    fn conflict_copy_device_name_with_spaces() {
        let p = RelativePath::new("photo.jpg");
        let at = ts(2025, 12, 31, 23, 59, 59);
        let copy = conflict_copy_path(&p, "My Work PC", at);
        assert_eq!(
            copy.as_str(),
            "photo (conflicted copy from My Work PC 2025-12-31 23-59-59).jpg"
        );
    }

    // T053-5: Two calls with different timestamps produce different paths.
    #[test]
    fn conflict_copy_unique_per_timestamp() {
        let p = RelativePath::new("file.txt");
        let a = conflict_copy_path(&p, "dev", ts(2024, 1, 1, 12, 0, 0));
        let b = conflict_copy_path(&p, "dev", ts(2024, 1, 1, 12, 0, 1));
        assert_ne!(a.as_str(), b.as_str());
    }

    // T053-6: NewestWins chooses local when local is newer.
    #[tokio::test]
    async fn resolver_newest_wins_prefers_local() {
        use crate::remote::mock::MockRemoteClient;
        use crate::types::{ConflictPolicy, PairId};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"local").unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/f.txt", b"remote").await;

        let record = make_record(
            "f.txt",
            ConflictPolicy::NewestWins,
            ts(2024, 1, 2, 0, 0, 0),
            ts(2024, 1, 1, 0, 0, 0),
        ); // local newer
        let (tx, _rx) = mpsc::channel(8);
        let journal = crate::journal::sqlite::SqliteJournal::open(&format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("j.db").display()
        ))
        .await
        .unwrap();

        let outcome = resolve(
            &record,
            &PairId::new(),
            &LocalPath::new(dir.path()),
            &RemotePath::new("remote/"),
            &client,
            &journal,
            "testdev",
            None,
            tx,
        )
        .await
        .unwrap();

        assert_eq!(outcome.resolution, ConflictResolution::KeptLocal);
    }

    // T053-7: NewestWins chooses remote when remote is newer.
    #[tokio::test]
    async fn resolver_newest_wins_prefers_remote() {
        use crate::remote::mock::MockRemoteClient;
        use crate::types::{ConflictPolicy, PairId};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("f.txt"), b"local").unwrap();
        let client = MockRemoteClient::new();
        client.seed("remote/f.txt", b"remote-newer").await;

        let record = make_record(
            "f.txt",
            ConflictPolicy::NewestWins,
            ts(2024, 1, 1, 0, 0, 0),
            ts(2024, 1, 2, 0, 0, 0),
        ); // remote newer
        let (tx, _rx) = mpsc::channel(8);
        let journal = crate::journal::sqlite::SqliteJournal::open(&format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("j2.db").display()
        ))
        .await
        .unwrap();

        let outcome = resolve(
            &record,
            &PairId::new(),
            &LocalPath::new(dir.path()),
            &RemotePath::new("remote/"),
            &client,
            &journal,
            "testdev",
            None,
            tx,
        )
        .await
        .unwrap();

        assert_eq!(outcome.resolution, ConflictResolution::KeptRemote);
    }

    // T053-8: Ask policy without user_choice returns Permanent error.
    #[tokio::test]
    async fn resolver_ask_without_choice_is_error() {
        use crate::remote::mock::MockRemoteClient;
        use crate::types::{ConflictPolicy, PairId};
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let client = MockRemoteClient::new();
        let record = make_record(
            "f.txt",
            ConflictPolicy::Ask,
            ts(2024, 1, 1, 0, 0, 0),
            ts(2024, 1, 1, 0, 0, 0),
        );
        let (tx, _rx) = mpsc::channel(8);
        let journal = crate::journal::sqlite::SqliteJournal::open(&format!(
            "sqlite://{}?mode=rwc",
            dir.path().join("j3.db").display()
        ))
        .await
        .unwrap();

        let err = resolve(
            &record,
            &PairId::new(),
            &LocalPath::new(dir.path()),
            &RemotePath::new("remote/"),
            &client,
            &journal,
            "dev",
            None, // no choice
            tx,
        )
        .await
        .unwrap_err();

        assert!(matches!(err, TransferError::Permanent(_)));
    }

    fn make_record(
        path: &str,
        policy: ConflictPolicy,
        local_mtime: DateTime<Utc>,
        remote_mtime: DateTime<Utc>,
    ) -> ConflictRecord {
        ConflictRecord {
            id: Uuid::new_v4().to_string(),
            pair_id: crate::types::PairId::new(),
            path: RelativePath::new(path),
            local_mtime,
            remote_mtime,
            local_size: 10,
            remote_size: 10,
            policy,
            resolution: None,
            detected_at: Utc::now(),
            resolved_at: None,
        }
    }
}
