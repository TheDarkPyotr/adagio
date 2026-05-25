use adagio_core::remote::mock::MockRemoteClient;
use adagio_core::transfer::{download::download_file, upload::upload_single, TransferOptions};
use adagio_core::types::{LocalPath, RemotePath};
use bytes::Bytes;
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use tempfile::TempDir;
use tokio::sync::mpsc;

const MB: usize = 1024 * 1024;

fn bench_upload_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("upload_throughput");

    for &size_mb in &[1usize, 10, 50] {
        let data: Vec<u8> = (0..size_mb * MB).map(|i| (i % 256) as u8).collect();
        let data_bytes = Bytes::from(data.clone());
        group.throughput(Throughput::Bytes((size_mb * MB) as u64));

        group.bench_with_input(
            BenchmarkId::new("upload_single", format!("{size_mb}MB")),
            &size_mb,
            |b, &_size_mb| {
                b.to_async(&rt).iter(|| async {
                    let dir = TempDir::new().unwrap();
                    let local_path = dir.path().join("payload.bin");
                    tokio::fs::write(&local_path, &data).await.unwrap();

                    let client = MockRemoteClient::new();
                    let local = LocalPath::new(&local_path);
                    let remote = RemotePath::new("bench/payload.bin");
                    let (tx, _rx) = mpsc::channel(8);

                    upload_single(&client, &local, &remote, &TransferOptions::default(), tx)
                        .await
                        .expect("upload should succeed");
                });
            },
        );
    }

    group.finish();
}

fn bench_download_throughput(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("download_throughput");

    for &size_mb in &[1usize, 10, 50] {
        let data: Vec<u8> = (0..size_mb * MB).map(|i| (i % 256) as u8).collect();
        group.throughput(Throughput::Bytes((size_mb * MB) as u64));

        group.bench_with_input(
            BenchmarkId::new("download_file", format!("{size_mb}MB")),
            &size_mb,
            |b, &_size_mb| {
                b.to_async(&rt).iter(|| {
                    let data = data.clone();
                    async move {
                        let dir = TempDir::new().unwrap();
                        let client = MockRemoteClient::new();
                        let remote = RemotePath::new("bench/data.bin");
                        client.seed(remote.as_str(), &data).await;

                        let local = LocalPath::new(dir.path().join("out.bin"));
                        let (tx, _rx) = mpsc::channel(8);

                        download_file(
                            &client,
                            &remote,
                            &local,
                            None,
                            &TransferOptions::default(),
                            tx,
                        )
                        .await
                        .expect("download should succeed");
                    }
                });
            },
        );
    }

    group.finish();
}

criterion_group!(benches, bench_upload_throughput, bench_download_throughput);
criterion_main!(benches);
