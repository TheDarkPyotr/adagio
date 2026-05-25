use adagio_core::cycle::reconciler::reconcile;
use adagio_core::types::{
    Checksum, ChecksumAlgorithm, ConflictPolicy, JournalEntry, LocalItem, PairId, RelativePath,
    RemoteItem, SyncStatus,
};
use chrono::Utc;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};

fn make_local(i: usize) -> LocalItem {
    LocalItem {
        path: RelativePath::new(format!("dir{}/file{}.txt", i / 100, i)),
        size: 1024,
        mtime: Utc::now(),
        checksum: Some(Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: format!("{:064x}", i),
        }),
        is_dir: false,
    }
}

fn make_remote(i: usize) -> RemoteItem {
    RemoteItem {
        path: RelativePath::new(format!("dir{}/file{}.txt", i / 100, i)),
        file_id: format!("fid{i}"),
        etag: format!("etag{i}"),
        size: 1024,
        mtime: Utc::now(),
        checksum: Some(Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: format!("{:064x}", i),
        }),
        is_dir: false,
    }
}

fn make_journal(i: usize) -> JournalEntry {
    JournalEntry {
        pair_id: PairId::new(),
        path: RelativePath::new(format!("dir{}/file{}.txt", i / 100, i)),
        file_id: Some(format!("fid{i}")),
        etag: Some(format!("etag{i}")),
        checksum: Some(Checksum {
            algorithm: ChecksumAlgorithm::Sha256,
            value: format!("{:064x}", i),
        }),
        size: 1024,
        mtime_local: Some(Utc::now()),
        mtime_remote: Some(Utc::now()),
        status: SyncStatus::Synced,
        error_message: None,
        retry_count: 0,
        updated_at: Utc::now(),
    }
}

fn bench_reconcile_100k(c: &mut Criterion) {
    let n: usize = 100_000;
    let local: Vec<LocalItem> = (0..n).map(make_local).collect();
    let remote: Vec<RemoteItem> = (0..n).map(make_remote).collect();
    let journal: Vec<JournalEntry> = (0..n).map(make_journal).collect();

    c.bench_with_input(
        BenchmarkId::new("reconcile", "100k_unchanged"),
        &n,
        |b, _| {
            b.iter(|| {
                reconcile(
                    black_box(&local),
                    black_box(&remote),
                    black_box(&journal),
                    ConflictPolicy::NewestWins,
                )
            })
        },
    );
}

fn bench_reconcile_all_new(c: &mut Criterion) {
    let n: usize = 10_000;
    let local: Vec<LocalItem> = (0..n).map(make_local).collect();
    let remote: Vec<RemoteItem> = (0..n).map(make_remote).collect();
    // Empty journal: all items are new.

    c.bench_with_input(BenchmarkId::new("reconcile", "10k_all_new"), &n, |b, _| {
        b.iter(|| {
            reconcile(
                black_box(&local),
                black_box(&remote),
                black_box(&[]),
                ConflictPolicy::NewestWins,
            )
        })
    });
}

criterion_group!(benches, bench_reconcile_100k, bench_reconcile_all_new);
criterion_main!(benches);
