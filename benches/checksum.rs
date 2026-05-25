use criterion::{criterion_group, criterion_main, Criterion};

fn bench_sha256(c: &mut Criterion) {
    // SHA-256 throughput benchmarks added in Phase 5 (US4).
    c.bench_function("sha256_noop", |b| b.iter(|| {}));
}

criterion_group!(benches, bench_sha256);
criterion_main!(benches);
