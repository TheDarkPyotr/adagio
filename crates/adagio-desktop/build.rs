fn main() {
    // Tauri's build script validates that every externalBin path exists before
    // it will compile. Real binaries are only needed at `tauri build` time, but
    // `cargo test` / `cargo clippy` trigger this check too. Create zero-byte
    // stubs so those commands work without a prior `cargo build --release`.
    let target = std::env::var("TARGET").unwrap_or_default();
    let binaries = std::path::Path::new("binaries");
    std::fs::create_dir_all(binaries).ok();
    for name in &["adagio-daemon", "adagio-cli"] {
        let stub = binaries.join(format!("{name}-{target}"));
        if !stub.exists() {
            std::fs::File::create(&stub).ok();
        }
    }

    tauri_build::build();
}
