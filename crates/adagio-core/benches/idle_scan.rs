use adagio_core::detection::local::{scan_local_with_stats, CachedLocalEntry};
use adagio_core::types::LocalPath;
use criterion::{criterion_group, criterion_main, Criterion};
use std::collections::HashMap;
use tempfile::TempDir;

/// Populate a temp directory with `n` small text files.
fn make_tree(dir: &TempDir, n: usize) {
    for i in 0..n {
        let subdir = dir.path().join(format!("sub{}", i % 10));
        std::fs::create_dir_all(&subdir).unwrap();
        std::fs::write(subdir.join(format!("file{i}.txt")), format!("content {i}")).unwrap();
    }
}

fn bench_idle_scan(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let dir = TempDir::new().unwrap();
    make_tree(&dir, 1000);
    let root = LocalPath::new(dir.path());

    // First scan: builds the cache and computes all checksums.
    let (first_items, first_stats) = rt
        .block_on(scan_local_with_stats(&root, &HashMap::new()))
        .expect("first scan should succeed");

    assert_eq!(
        first_stats.checksum_recomputed, 1000,
        "first scan should recompute all 1000 checksums"
    );

    // Build the cache from the first scan results.
    let cache: HashMap<_, _> = first_items
        .iter()
        .filter(|i| !i.is_dir)
        .map(|i| {
            (
                i.path.clone(),
                CachedLocalEntry {
                    mtime: i.mtime,
                    size: i.size,
                    checksum: i.checksum.clone(),
                },
            )
        })
        .collect();

    // Steady-state idle scan: everything hits the cache.
    let mut group = c.benchmark_group("idle_scan");
    group.bench_function("1000_unchanged_files", |b| {
        b.iter(|| {
            let (_, stats) = rt
                .block_on(scan_local_with_stats(&root, &cache))
                .expect("idle scan should succeed");
            assert_eq!(
                stats.checksum_recomputed, 0,
                "idle scan must recompute zero checksums"
            );
            assert_eq!(stats.checksum_from_cache, 1000);
        });
    });
    group.finish();
}

criterion_group!(benches, bench_idle_scan);
criterion_main!(benches);
