# Data Model: Bandwidth Throttling (009)

**Date**: 2026-05-30 | **Feature**: `009-bandwidth-throttling`

---

## Modified: `Account` (adagio-core/src/types.rs)

Add two fields with sensible defaults:

```rust
pub struct Account {
    pub id: AccountId,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub keychain_service_key: String,
    pub created_at: DateTime<Utc>,
    // NEW — 0 = unlimited
    #[serde(default)]
    pub upload_limit_kbps: u64,
    #[serde(default)]
    pub download_limit_kbps: u64,
}
```

---

## Modified: `SavedAccount` (adagio-desktop/src/config/mod.rs)

Mirror the same fields on the persistence type:

```rust
pub struct SavedAccount {
    pub id: String,
    pub display_name: String,
    pub server_url: String,
    pub username: String,
    pub keychain_service_key: String,
    // NEW — 0 = unlimited; #[serde(default)] ensures backward compat
    #[serde(default)]
    pub upload_limit_kbps: u64,
    #[serde(default)]
    pub download_limit_kbps: u64,
}
```

No schema migration is needed — existing `config.json` files read cleanly because
`serde(default)` fills 0 for the absent fields.

---

## New: `ThroughputMeter` (adagio-core/src/bandwidth.rs)

A rolling-window byte-rate calculator:

```rust
/// Tracks bytes transferred over a sliding 10-second window and computes
/// an instantaneous rate in bytes/second.
pub struct ThroughputMeter {
    window: VecDeque<(std::time::Instant, u64)>,
    window_secs: u64,  // default 10
}

impl ThroughputMeter {
    pub fn new() -> Self;
    /// Record that `bytes` bytes were transferred at this instant.
    pub fn record(&mut self, bytes: u64);
    /// Return the current rate in bytes/second (rolling window average).
    pub fn rate_bytes_per_sec(&mut self) -> u64;
}
```

---

## New: `BandwidthStatus` (adagio-ipc/src/types.rs — response shape)

The JSON response payload returned by `GetBandwidthStatus`:

```rust
// Serialises as a plain JSON object — no DaemonResponse wrapper needed.
// Example: {"upload_limit_kbps":500,"download_limit_kbps":0,
//           "upload_bytes_per_sec":48000,"download_bytes_per_sec":0}
struct BandwidthStatus {
    upload_limit_kbps: u64,      // 0 = unlimited
    download_limit_kbps: u64,    // 0 = unlimited
    upload_bytes_per_sec: u64,   // rolling 10s average
    download_bytes_per_sec: u64, // rolling 10s average
}
```

---

## Modified: `DaemonRequest` (adagio-ipc/src/types.rs)

Three new variants:

```rust
/// Return current bandwidth limits and live throughput for an account.
GetBandwidthStatus { account_id: Option<String> },

/// Set upload and/or download limits for an account (0 = unlimited).
SetBandwidthLimits {
    account_id: Option<String>,
    upload_kbps: u64,
    download_kbps: u64,
},

/// Remove all bandwidth limits for an account.
ClearBandwidthLimits { account_id: Option<String> },
```

---

## Modified: `upload_single` signature (adagio-core/src/transfer/upload.rs)

```rust
pub async fn upload_single(
    client: &dyn RemoteClient,
    local_path: &LocalPath,
    remote_path: &RemotePath,
    opts: &TransferOptions,
    progress: mpsc::Sender<TransferProgress>,
    throttle: Option<Arc<TokenBucket>>,  // NEW
) -> Result<UploadResult, TransferError>
```

Internal: reads file in 64 KB chunks; calls `throttle.acquire(chunk_size).await`
before yielding each chunk.

---

## Modified: `download_file` signature (adagio-core/src/transfer/download.rs)

```rust
pub async fn download_file(
    client: &dyn RemoteClient,
    remote_path: &RemotePath,
    local_path: &LocalPath,
    expected_checksum: Option<Checksum>,
    opts: &TransferOptions,
    progress: mpsc::Sender<TransferProgress>,
    throttle: Option<Arc<TokenBucket>>,  // NEW
) -> Result<DownloadResult, TransferError>
```

Internal: applies `throttle.acquire(chunk_size).await` at each write boundary.

---

## New: `BandwidthCommand` (adagio-cli/src/cli.rs)

```rust
#[derive(Subcommand)]
pub enum BandwidthCommand {
    /// Show configured limits and current throughput.
    Status,
    /// Set upload and/or download speed limits.
    Set {
        /// Upload limit in Kbps (0 = unlimited).
        #[arg(long)]
        upload_kbps: Option<u64>,
        /// Download limit in Kbps (0 = unlimited).
        #[arg(long)]
        download_kbps: Option<u64>,
    },
    /// Remove all bandwidth limits.
    Clear,
}
```

---

## No database changes

All bandwidth configuration is stored in `config.json` via the existing
`SavedAccount` serialisation path. No new SQLite tables or migrations.
