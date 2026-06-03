/// Benchmark AES-128-GCM encrypt/decrypt throughput (T057).
///
/// Success criterion: 10 MB encrypt + decrypt completes in ≤ 100 ms on target
/// hardware. Run with `cargo bench -p adagio-e2ee`.
use adagio_e2ee::cipher::{decrypt_file, encrypt_file};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};

fn bench_encrypt_decrypt(c: &mut Criterion) {
    let sizes_kb: &[usize] = &[64, 1024, 10 * 1024]; // 64 KB, 1 MB, 10 MB

    let mut group = c.benchmark_group("aes_128_gcm");
    for &kb in sizes_kb {
        let bytes = kb * 1024;
        let plaintext = vec![0u8; bytes];
        group.throughput(Throughput::Bytes(bytes as u64));

        group.bench_with_input(BenchmarkId::new("encrypt", kb), &plaintext, |b, pt| {
            b.iter(|| {
                let _ = encrypt_file(pt).expect("encrypt must succeed");
            });
        });

        // Pre-encrypt once so the decrypt bench has stable input.
        let (key, nonce, tag, ct) = encrypt_file(&plaintext).unwrap();
        group.bench_with_input(
            BenchmarkId::new("decrypt", kb),
            &(key, nonce, tag, ct),
            |b, (k, n, t, c)| {
                b.iter(|| {
                    let _ = decrypt_file(k, n, t, c).expect("decrypt must succeed");
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_encrypt_decrypt);
criterion_main!(benches);
