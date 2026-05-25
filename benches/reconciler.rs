use criterion::{criterion_group, criterion_main, Criterion};

fn bench_placeholder(c: &mut Criterion) {
    // Reconciler benchmarks added in Phase 4 (US2).
    c.bench_function("reconciler_noop", |b| b.iter(|| {}));
}

criterion_group!(benches, bench_placeholder);
criterion_main!(benches);
