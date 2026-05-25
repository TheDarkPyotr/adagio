use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sha2::{Digest, Sha256};

fn sha256_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("sha256_streaming");

    for &size_mb in &[1usize, 10, 100] {
        let data: Vec<u8> = (0..size_mb * 1024 * 1024)
            .map(|i| (i % 256) as u8)
            .collect();

        group.throughput(Throughput::Bytes(data.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("digest", format!("{size_mb}MB")),
            &data,
            |b, d| {
                b.iter(|| {
                    let mut h = Sha256::new();
                    h.update(black_box(d));
                    h.finalize()
                })
            },
        );
    }

    group.finish();
}

criterion_group!(benches, sha256_throughput);
criterion_main!(benches);
