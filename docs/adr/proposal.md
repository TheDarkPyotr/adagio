# Implementation Plan: Lean, Reliable Nextcloud Desktop Sync Client

**Feature Branch**: `001-nextcloud-client`
**Companion spec**: `spec.md`
**Status**: Draft
**Owner**: Luca G. Pinta

---

## Technical Context

| Field                    | Value |
|--------------------------|-------|
| Primary language         | Rust (stable, MSRV 1.83) |
| Runtime                  | Native, no JIT/VM; static link of musl on Linux release builds |
| Async runtime            | `tokio` (multi-thread, current-thread for the daemon's event loop) |
| HTTP/WebDAV              | `reqwest` (rustls-only, no OpenSSL), `quick-xml` for DAV bodies, hand-rolled WebDAV verbs (PROPFIND, MKCOL, MOVE, COPY) |
| Local state              | SQLite via `rusqlite` (bundled), WAL mode, single writer |
| Cryptography             | `ring` for hashes/HMAC, `aes-gcm` (RustCrypto) for E2EE content, `ed25519-dalek` for signatures, `bip39` for mnemonic |
| GUI shell                | Tauri v2, system webview, native menus & tray |
| GUI frontend             | TypeScript + Svelte 5; no Electron, no bundled Chromium |
| CLI                      | `clap` v4 with derive, `--json` output via `serde_json` |
| IPC                      | gRPC over Unix domain socket (Linux/macOS) / named pipe (Windows), `tonic` server, `prost` messages |
| Logging                  | `tracing` + `tracing-subscriber` (JSON layer), `tracing-appender` for rotation |
| Credential storage       | `keyring-rs` (secret-service / Keychain / DPAPI), local AES-GCM file fallback for headless |
| Packaging                | MSI (WiX 4) on Windows; DMG + notarized `.app` on macOS; `.deb`, `.rpm`, Flatpak, AppImage on Linux; static musl tarball for headless |
| Scale targets            | Single account up to 1 M tracked paths; 100 GB working set; 10 concurrent connections; 5 simultaneous large transfers |
| Server compatibility     | Nextcloud server 28 LTS minimum; full feature use on 30+; E2EE v2.x requires server end_to_end_encryption app ≥ 2.0 |
| Supported clients (OS)   | Windows 10 (22H2) and 11; macOS 12 Monterey through current; Linux x86_64 and aarch64 with glibc 2.31+ or musl; headless Linux on Raspberry Pi 4/5 class hardware |

The Rust + Tauri + gRPC choice is deliberate: it gives a small binary (target < 30 MB total install on Linux excluding Qt), a strict process boundary between the sync engine and the UI, and a CLI that is the same binary as the daemon's IPC client. This is the structural lever that makes the "lean by default" and "CLI-first" requirements achievable rather than aspirational.

---

## Constitution Check

| Principle                            | How this plan satisfies it |
|--------------------------------------|----------------------------|
| Library-first, CLI-first             | Sync engine is a library crate (`ncsync-core`) consumed by both the daemon binary (`ncsyncd`) and the CLI binary (`ncsync`). GUI is a separate binary that talks to the daemon over IPC. |
| Test-first                           | Contract tests for the WebDAV client and IPC service generated from `.proto` and OpenAPI before implementation. Integration tests use a containerised Nextcloud server (AIO image) per CI run. |
| Observable                           | All IPC requests and sync operations emit `tracing` spans with stable event names; logs are JSON; bug-report bundle ships ready-to-attach. |
| Simplicity                           | One persistent process for sync; one persistent process for GUI; everything else is short-lived CLI invocations. No background scheduler outside the daemon. |
| No speculative features              | Every section below maps to a Functional Requirement ID from `spec.md`. |

---

## Architecture Overview

```
                +-----------------------------------------------------+
                |                       User Space                    |
                |                                                     |
                |   +-------------+        IPC (gRPC over UDS/pipe)   |
                |   |  GUI shell  | <----------------------------+    |
                |   |  (Tauri)    |                              |    |
                |   +-------------+                              |    |
                |                                                v    |
                |   +-------------+   stdin/stdout/exit code   +------+--------+
                |   |     CLI     | -------------------------> |               |
                |   | (ncsync)    | <------------------------- |   ncsyncd     |
                |   +-------------+                            |   (daemon)    |
                |                                              |               |
                |   +-------------+                            |  +---------+  |
                |   | File system | <--------- VFS hooks ---->  |  | sync   |  |
                |   |  + native   |                            |  | engine |  |
                |   |  file mgr   |                            |  +---------+  |
                |   +-------------+                            |  +---------+  |
                |                                              |  | WebDAV |  |
                |                                              |  | client |  |
                |                                              |  +---------+  |
                |                                              |  +---------+  |
                |                                              |  | E2EE   |  |
                |                                              |  +---------+  |
                |                                              +------+--------+
                |                                                     |
                +-----------------------------------------------------+
                                                                      |
                                                                  HTTPS / mTLS
                                                                      |
                                                                      v
                                                              +----------------+
                                                              |  Nextcloud     |
                                                              |  server        |
                                                              +----------------+
```

**Process model:**

- `ncsyncd` — long-running per-user daemon. Holds all credentials, the sync database, network sockets, and platform VFS handles. No GUI dependencies. Survives GUI close/restart.
- `ncsync` — CLI binary. Connects to the local `ncsyncd` over IPC, issues a command, prints the result, exits. Bundles the same `ncsync-core` library so it can also run a one-shot sync without a daemon (useful for cron jobs and CI).
- `ncsync-gui` — Tauri shell. Frontend (Svelte) talks to a thin Rust backend that proxies to `ncsyncd` over IPC. GUI process can be killed and respawned without disrupting sync.

**Why three processes:** the user can quit the GUI and sync continues; a GUI crash never corrupts the sync database; the CLI works on headless systems with no display server; resources are cleanly separable for measurement.

---

## Component Design

### 1. `ncsync-core` (library crate)

Public modules:

- `account` — account configuration, credential resolution, server capability discovery.
- `db` — SQLite schema, migrations, query helpers. Single connection per account, WAL.
- `dav` — WebDAV client: PROPFIND, GET, PUT, MKCOL, MOVE, COPY, DELETE, REPORT (file ID).
- `chunk` — Chunked upload v2 driver. MKCOL on `/dav/uploads/<uid>/<token>`, PUTs of numbered chunks 1..N (5 MB–5 GB), MOVE to destination with `Destination` and `OC-Total-Length` headers. Resumable via the token persisted in `pending_uploads`.
- `bulk` — Bulk upload driver for files under a server-advertised threshold (default 1 MB). `multipart/related` POST to `/dav/bulk`.
- `engine` — Reconciliation engine (see [Sync Algorithm](#sync-engine-algorithm)).
- `vfs` — Trait `VirtualFs` with three implementations: `windows::CfApi`, `macos::FileProvider`, `linux::Fuse3`. The trait abstracts placeholder creation, hydration request handling, and dehydration.
- `e2ee` — End-to-end encryption per RFC 2.x. AES-256-GCM content encryption, RSA-2048 or P-256 key pair, BIP-39 mnemonic, server-signed certificates (TOFU then signed).
- `share` — Activity feed via OCS API `/ocs/v2.php/cloud/activity` and share metadata via `/ocs/v2.php/apps/files_sharing/api/v1/shares`.
- `peer` — LAN-peer discovery (mDNS) and transfer protocol.
- `policy` — Bandwidth caps, metered/battery/SSID-aware scheduling.

### 2. `ncsyncd` (daemon binary)

Loop:

1. On start, open IPC listener with `0600` permissions (UDS) or per-user pipe DACL (Windows).
2. Load accounts from config; for each, open SQLite, resolve credentials, fetch server capabilities (`/ocs/v2.php/cloud/capabilities`), reconcile schema version.
3. Spawn one `tokio::task` per account hosting an `engine::Sync` loop driven by:
   - filesystem watcher events (notify-rs on all platforms with platform-specific backends),
   - periodic poll (default 30 s, jittered ±5 s),
   - VFS hydration requests,
   - explicit IPC commands.
4. Process IPC requests on a separate task. Mutate per-account state via `tokio::sync::mpsc` channels — no shared mutable state across accounts.
5. Emit `tracing` events to JSON files under platform log dir; rotation handled by `tracing-appender`.

### 3. `ncsync` (CLI binary)

Subcommands (all support `--json`):

```
ncsync account add | list | remove | test
ncsync sync [path]              # one-shot sync, blocks until idle
ncsync status [account]         # current state, last error, queue depth
ncsync pause | resume [account]
ncsync conflict list | resolve <path> --keep local|remote|both
ncsync exclude add <pattern> | list | remove <pattern>
ncsync vfs hydrate <path> | dehydrate <path> | policy set ...
ncsync e2ee init | unlock | rotate | export-mnemonic
ncsync log tail [--follow]
ncsync bug-report                # builds tar.gz under cwd
ncsync daemon start | stop | reload
```

Exit codes: `0` success, `2` user error, `3` transient (retry possible), `4` permanent failure, `5` daemon unreachable.

### 4. `ncsync-gui` (Tauri shell)

- Tauri commands are 1:1 wrappers around IPC calls; the Svelte frontend never touches the file system or network directly.
- Tray icon shows daemon state pulled from a single `Status` server-streaming RPC.
- All settings panels are forms that submit a `Settings` patch to the daemon; the daemon validates and persists.

---

## Data Model

SQLite, one database per account at:
- Linux/macOS: `$XDG_STATE_HOME/ncsync/<account-id>/state.db`
- Windows: `%LOCALAPPDATA%\ncsync\<account-id>\state.db`

```sql
-- Files known to the engine.
CREATE TABLE nodes (
    id              INTEGER PRIMARY KEY,
    path            TEXT NOT NULL UNIQUE,        -- POSIX-style, relative to root
    parent_id       INTEGER REFERENCES nodes(id),
    kind            INTEGER NOT NULL,            -- 0 file, 1 dir
    remote_etag     TEXT,                        -- WebDAV ETag of last known remote
    remote_fileid   TEXT,                        -- OC-FileId (immutable across rename)
    remote_mtime    INTEGER,                     -- seconds since epoch
    remote_size     INTEGER,
    local_inode     INTEGER,                     -- inode/FileId for rename detection
    local_mtime     INTEGER,
    local_size      INTEGER,
    local_hash      BLOB,                        -- BLAKE3-256, NULL for dirs and placeholders
    placeholder     INTEGER NOT NULL DEFAULT 0,  -- 0 hydrated, 1 placeholder, 2 pinned
    e2ee_folder_id  INTEGER REFERENCES e2ee_folders(id),
    state           INTEGER NOT NULL,            -- see SyncState enum
    last_synced_at  INTEGER
);
CREATE INDEX nodes_parent ON nodes(parent_id);
CREATE INDEX nodes_state  ON nodes(state) WHERE state <> 0;

CREATE TABLE pending_uploads (
    node_id         INTEGER PRIMARY KEY REFERENCES nodes(id),
    upload_token    TEXT NOT NULL,               -- UUID under /dav/uploads/<uid>/<token>
    chunk_size      INTEGER NOT NULL,
    chunks_done     BLOB NOT NULL,               -- packed bitmap, bit N = chunk N+1 uploaded
    started_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL             -- server expires upload dir after 24h
);

CREATE TABLE conflicts (
    node_id         INTEGER PRIMARY KEY REFERENCES nodes(id),
    local_hash      BLOB NOT NULL,
    remote_etag     TEXT NOT NULL,
    detected_at     INTEGER NOT NULL,
    resolution      INTEGER                       -- NULL while unresolved
);

CREATE TABLE excludes (
    id              INTEGER PRIMARY KEY,
    scope_path      TEXT NOT NULL DEFAULT '',     -- '' = global, else folder prefix
    pattern         TEXT NOT NULL,
    negate          INTEGER NOT NULL DEFAULT 0,   -- 1 if leading '!'
    ordinal         INTEGER NOT NULL              -- preserves authoring order
);

CREATE TABLE e2ee_folders (
    id              INTEGER PRIMARY KEY,
    root_node_id    INTEGER NOT NULL REFERENCES nodes(id),
    metadata_ver    TEXT NOT NULL,                -- "2.0", "2.1", …
    counter         INTEGER NOT NULL DEFAULT 0,   -- monotonic for metadata anti-rollback
    metadata_blob   BLOB NOT NULL                 -- cached decrypted metadata.json
);

CREATE TABLE settings (
    key             TEXT PRIMARY KEY,
    value           TEXT NOT NULL                 -- JSON
);

CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
```

**SyncState enum:**

```
0 IN_SYNC
1 LOCAL_NEW
2 LOCAL_MODIFIED
3 LOCAL_DELETED
4 REMOTE_NEW
5 REMOTE_MODIFIED
6 REMOTE_DELETED
7 CONFLICT
8 UPLOAD_IN_PROGRESS
9 DOWNLOAD_IN_PROGRESS
10 ERROR_TRANSIENT
11 ERROR_PERMANENT
12 EXCLUDED
```

Migrations are forward-only, numbered, applied in a transaction. The schema version is checked at daemon start; refusal to start on downgrade with a clear log line.

---

## Sync Engine Algorithm

### Reconciliation cycle (per account, per tick)

1. **Capability snapshot.** Cache server capabilities for 1 hour; refresh on auth error or 24 h elapsed.
2. **Remote scan.** `PROPFIND Depth:infinity` on the root, or `Depth:1` for sub-folders flagged dirty by previous activity polling. The server may reject `infinity` for large trees; fall back to BFS with `Depth:1`. Request these props: `getlastmodified`, `getetag`, `getcontentlength`, `resourcetype`, `oc:fileid`, `oc:permissions`, `nc:has-preview`, `nc:is-encrypted`.
3. **Local scan.** Walk the sync root with `walkdir`, honouring excludes. For placeholders, do not stat content; trust the local DB for hash.
4. **Diff.** Join remote scan against `nodes` table by `oc:fileid` (preferred, survives rename) falling back to path. Same for local against `nodes` by inode then path.
5. **State assignment.** For each node, compute state from the matrix:

   | local         | remote          | state                    |
   |---------------|-----------------|--------------------------|
   | unchanged     | unchanged       | IN_SYNC                  |
   | new           | absent          | LOCAL_NEW                |
   | modified      | unchanged       | LOCAL_MODIFIED           |
   | deleted       | unchanged       | LOCAL_DELETED            |
   | absent        | new             | REMOTE_NEW               |
   | unchanged     | modified        | REMOTE_MODIFIED          |
   | unchanged     | deleted         | REMOTE_DELETED           |
   | modified      | modified        | CONFLICT                 |
   | deleted       | deleted         | (drop row)               |
   | modified      | deleted         | CONFLICT (keep local)    |
   | deleted       | modified        | CONFLICT (keep remote)   |

6. **Operation queue.** Build a topologically ordered op list: directory creates before file writes, file deletes before parent-dir deletes, etc. Operations are serialised into `nodes.state` so a crash mid-cycle resumes safely.
7. **Execute.** Tasks fanned out with a bounded `tokio::sync::Semaphore` (default 4 transfers).
8. **Settle.** Commit DB updates per-op in their own transactions to bound rollback cost.

### Rename detection

- Local: same inode/FileId, different path → rename. Issue MOVE.
- Remote: same `oc:fileid`, different path → rename. Apply rename locally.
- Cross: heuristic on (content-hash, size) within the same parent rename window (5 s) — only when fileid is missing on legacy servers.

### Retry & backoff

- Transient network/5xx: exponential backoff base 2 s, cap 5 min, max 6 attempts, then state moves to `ERROR_TRANSIENT` and is retried on next tick.
- Per-host token bucket on requests/sec to prevent the runaway-loop pathology seen in 4.0.8 — hard ceiling 50 req/s per account, 5 req/s per resource path. If the bucket empties the engine refuses to issue more requests for that resource until the next reconciliation cycle.

---

## WebDAV Transport

### Endpoints

| Purpose                  | Path                                                       |
|--------------------------|------------------------------------------------------------|
| Files (CRUD)             | `/remote.php/dav/files/<uid>/<path>`                       |
| Chunked upload v2        | `/remote.php/dav/uploads/<uid>/<token>/`                   |
| Bulk upload (small files)| `/remote.php/dav/bulk`                                     |
| Trashbin                 | `/remote.php/dav/trashbin/<uid>/`                          |
| Versions                 | `/remote.php/dav/versions/<uid>/versions/<fileid>`         |
| Capabilities             | `/ocs/v2.php/cloud/capabilities`                           |
| Activity                 | `/ocs/v2.php/cloud/activity`                               |
| Shares                   | `/ocs/v2.php/apps/files_sharing/api/v1/shares`             |
| E2EE keys & metadata     | `/ocs/v2.php/apps/end_to_end_encryption/api/v2/…`          |

### Chunked upload v2 driver

Reflecting the published server contract: chunks numbered 1..10000, each 5 MB–5 GB except the last, `Destination` header required on every PUT, `OC-Total-Length` on every PUT for quota check, server expires the upload directory after 24 h of inactivity.

```
1. MKCOL /dav/uploads/<uid>/<token>
2. For each chunk i in 1..N:
     PUT  /dav/uploads/<uid>/<token>/<i>
          Destination: https://server/.../files/<uid>/<dest>
          OC-Total-Length: <total>
          Content-Length: <chunk-size>
3. MOVE /dav/uploads/<uid>/<token>/.file -> /dav/files/<uid>/<dest>
        Destination: https://server/.../files/<uid>/<dest>
        OC-Total-Length: <total>
```

Resume: on restart, `PROPFIND Depth:1` on the upload directory enumerates chunks already present; the bitmap in `pending_uploads.chunks_done` is reconciled before continuing.

Failure modes handled explicitly: `507 Insufficient Storage` → mark account quota-exceeded, surface to UI, do not retry until next user action; `404` on MOVE → upload directory expired, restart upload; `412 Precondition Failed` on `If-Match` for MOVE → server-side change, downgrade to conflict.

### Bulk upload driver

For files smaller than the server-advertised bulk threshold (capability `bulkupload`), pack up to 100 files or 100 MB per `multipart/related` POST to `/dav/bulk`. Per-file headers include `X-File-Path`, `X-File-Mtime`, `X-OC-Mtime`, `OC-Total-Length`, `X-File-MD5`.

### Concurrency rules

- Per account: at most 4 simultaneous transfers (configurable).
- Per file: never two simultaneous operations.
- Reads (GET, PROPFIND) and writes (PUT, MOVE) share the same connection pool, capped at 8 connections per host.
- HTTP/2 is preferred and negotiated via ALPN; we never multiplex more than 10 streams on one connection.

---

## Virtual Files Per Platform

The `VirtualFs` trait:

```rust
pub trait VirtualFs: Send + Sync {
    fn register_root(&self, root: &Path, account_id: &AccountId) -> Result<()>;
    fn create_placeholder(&self, path: &Path, meta: PlaceholderMeta) -> Result<()>;
    fn dehydrate(&self, path: &Path) -> Result<()>;
    fn on_hydration_request(&self, cb: HydrationCb);
    fn on_file_change(&self, cb: ChangeCb);
}
```

### Windows — Cloud Files API

- Use the `cloudfilter`/`projfs` Win32 API via the `windows` crate.
- Register a sync root with `CfRegisterSyncRoot`; the registration carries an icon, name, account hint.
- Placeholders created with `CfCreatePlaceholders` per directory batch.
- Hydration: respond to `CF_CALLBACK_TYPE_FETCH_DATA` with chunked transfer through `CfExecute(CF_OPERATION_TYPE_TRANSFER_DATA)`.
- Pinned/dehydrated state managed via `CfSetPinState` and `CfDehydratePlaceholder`.
- File ID stability via `CfGetPlaceholderInfo`.

### macOS — File Provider Extension

- Out-of-process app extension (`NSFileProviderExtension`) loaded by `FileProviderD`. Cannot share memory with the daemon; communicate over an XPC bridge to a small helper that talks to `ncsyncd` over the same IPC socket.
- `NSFileProviderItem`, `NSFileProviderEnumerator`, `NSFileProviderReplicatedExtension` (replicated mode is required for full parity with iCloud Drive).
- Hydration via `fetchContents(for:request:completionHandler:)`.
- The bundle is signed with the Developer ID + notarized; entitlements include `com.apple.developer.fileprovider.testing-mode` and `com.apple.security.app-sandbox`.
- Eviction uses `NSFileProviderManager.evictItem`.

### Linux — FUSE3

- Daemon links `libfuse3` via the `fuser` crate or direct FFI.
- The mount point is `~/Nextcloud (vfs)` by default; `/etc/fuse.conf` must allow `user_allow_other` if shared.
- Inode-stable mapping: `oc:fileid` → 64-bit inode; collisions resolved by a top-bit reserved namespace.
- Hydration: a `read` callback below threshold reads from local cache; above threshold downloads from server with `Range:` requests and caches to a disk-backed file. Files under a configurable size (default 256 KB) are downloaded eagerly on `open`.
- Dehydration: a daemon timer evicts hydrated files whose `atime` is older than the policy threshold, replacing them with sparse placeholders (truncate to size, drop content).
- xattr support: `user.nextcloud.fileid`, `user.nextcloud.etag`, `user.nextcloud.state`.

All three implementations report the same `PlaceholderMeta` to the engine: `(fileid, size, mtime, hash_if_known)`. The engine has zero platform-specific code.

---

## End-to-End Encryption Implementation

Targets the published Nextcloud E2EE RFC, metadata format ≥ 2.0 (server `end_to_end_encryption` ≥ 2.0). Metadata 1.x is read-only — the client can decrypt legacy folders but always re-uploads in 2.x.

### Setup

1. Generate RSA-2048 key pair (the server still requires RSA for backward compatibility; ECC support pending RFC 2.2).
2. Generate a 12-word BIP-39 mnemonic; derive a 256-bit key via PBKDF2-HMAC-SHA256 with 600 000 iterations, salt = UID.
3. Encrypt the private key with AES-256-GCM under the derived key; upload to `/ocs/v2.php/apps/end_to_end_encryption/api/v2/private-key`.
4. Generate a CSR; POST to `/ocs/v2.php/apps/end_to_end_encryption/api/v2/server-key/csr`; receive a server-signed certificate.
5. The wizard then **requires** the user to re-enter a randomly chosen subset of 3 of the 12 mnemonic words before completing — satisfying FR-09 acceptance criterion.

### Per-folder encryption

- Create a metadata file `metadata.json` per E2EE root, containing for each file: encrypted filename, per-file AES-256-GCM key, nonce, mime type, mtime, size, authentication tag.
- File content encrypted with its per-file key, AES-256-GCM, 12-byte nonce per write; ciphertext written as `<random-id>.bin` at the server.
- Metadata encrypted as a single blob with the folder metadata key; folder metadata key wrapped with each authorised user's public key (key bag).
- Anti-rollback: `metadata.json` includes a monotonic `counter`; the client refuses to apply metadata where `counter` decreases.
- Locking: before any write, `POST /lock/<fileid>`; on completion, `DELETE /lock/<fileid>`. Locks expire server-side after 60 s — the client renews every 30 s for long uploads.

### Device pairing (QR)

When a second device pairs:

1. New device generates a short-lived X25519 key pair; encodes its public key in a QR code.
2. Existing device scans QR, performs X25519 ECDH, derives a session key via HKDF-SHA256.
3. Existing device encrypts the mnemonic under the session key and uploads to the server's pairing endpoint (`/ocs/.../pairing/<token>`), which acts as a relay only.
4. New device fetches the blob, decrypts, derives the same private key, completes setup.

The server never sees plaintext mnemonic at any stage.

### Failure surface contract

Any failed E2EE operation surfaces an error of the form:

```json
{
  "phase": "key_generation" | "metadata_upload" | "metadata_decrypt" | "content_encrypt" | "content_decrypt" | "lock_acquire" | "pairing_handshake",
  "code":  "...",            // stable machine code
  "hint":  "...",            // short user-facing message
  "next":  ["..."]            // ordered list of recovery actions
}
```

This shape is enforced by the daemon and the GUI never invents a generic message of its own.

---

## Network & Bandwidth Layer

A single `Throttler` sits between the `dav` client and `reqwest`. It exposes:

```rust
async fn read(&self, n: usize) -> usize;   // downstream
async fn write(&self, n: usize) -> usize;  // upstream
```

Implementation: leaky bucket per direction per account; configured rates respected over any 10 s window (FR-06 acceptance).

### Network awareness

- Metered detection: Windows `INetworkConnectionCost`, macOS `nw_path_is_constrained`, Linux NetworkManager `org.freedesktop.NetworkManager.Device.Metered` via zbus.
- Interface filter: poll `getifaddrs` / NetworkManager every 10 s; match against the user's allowlist (interface name, Wi-Fi SSID, VPN name). When the active interface is not allowed, all transfers are suspended (not just throttled to zero — full pause to release sockets).
- Battery awareness: `org.freedesktop.UPower` on Linux, `IOPSGetProvidingPowerSourceType` on macOS, `GetSystemPowerStatus` on Windows. State change is observed via DBus / NSNotification / WM_POWERBROADCAST.

### Connection management

- HTTP/2, ALPN-negotiated; HTTP/1.1 fallback.
- Per-host connection pool: max 8 idle, idle timeout 30 s.
- Keepalive ping every 60 s on idle HTTP/2 connections.
- TLS via `rustls`; certificate validation against the platform trust store via `rustls-platform-verifier`. Self-signed servers require explicit trust pinning via `ncsync account add --pin-cert <fingerprint>`.

---

## LAN-Peer Protocol (FR-11)

Off by default. When enabled per account:

1. **Discovery.** mDNS service `_ncsync._tcp.local`, TXT records carry `account_fingerprint=<hex>` (HMAC of account UUID with server's per-account session key) and `protocol=1`.
2. **Authentication.** Peer presents a token signed by the server's per-account session key, valid for 1 hour, fetched from `/ocs/v2.php/apps/end_to_end_encryption/api/v2/peer-token` (when E2EE is on) or a dedicated peer-token endpoint. The token must validate against the user's account on the receiving end — peers from a different account on the same machine are rejected.
3. **Transfer.** TLS over the local socket using the same chain as HTTPS to the server; peer must present a certificate the server has previously signed. File content transferred as raw bytes with BLAKE3 hash validation against the server-reported hash.
4. **Fallback.** Any authentication, integrity, or transport failure → falls back transparently to server fetch. Never emits a user-visible error for peer failures.

Server is always the source of truth for state; peers only short-circuit the byte transfer.

---

## Conflict Resolution

When a node enters `CONFLICT`:

1. Both versions are preserved locally: the original path keeps the local version; the remote version is fetched to `<basename> (conflicted YYYY-MM-DD HHMM by <device>).<ext>` adjacent to it.
2. A `conflicts` row is inserted.
3. On user action via CLI (`ncsync conflict resolve`) or GUI:
   - For text-like files (MIME `text/*`, `application/json`, `application/xml`, `application/javascript`, source code by extension): present a 3-pane diff. The "common ancestor" is the last known synced version stored as a versioned `.ncsync/conflict-base/<fileid>` cache (entries pruned after 7 days).
   - For binaries: present `local size/mtime/hash` vs `remote size/mtime/hash` with three buttons.
4. Resolution commits a single MOVE (rename) and/or PUT, then deletes the conflict row and the conflicted copy.

While a conflict is unresolved on a node, the rest of the folder syncs normally — the engine treats `CONFLICT` as a per-node block, not a per-folder block.

---

## Security & Credential Storage

- Credentials at rest: never in plaintext config. `keyring-rs` on every platform; on Linux, falls back to a daemon-owned file at `$XDG_STATE_HOME/ncsync/credentials.enc` encrypted with AES-256-GCM under a key derived from `getpwuid(getuid())` + a randomly generated salt stored mode-`0600`. Headless setups must opt in to this fallback via `ncsync account add --headless-store`.
- App passwords (preferred over user passwords): the login flow performs OAuth2-style login flow v2 (`/index.php/login/v2`), receives an app password, never sees the user's actual password.
- TLS: minimum TLS 1.2, prefer 1.3; refuse to fall back to TLS 1.0/1.1; refuse compression.
- IPC socket: `0600` permissions; abstract sockets disabled on Linux to avoid kernel-namespace bypass. Each IPC frame carries a process credential check (`SO_PEERCRED`) — the daemon refuses connections from other UIDs.
- Logs: a `Redactor` layer in `tracing-subscriber` rewrites any field tagged `#[redact]` and any URL parameter named `token`, `password`, `share_url`, `mnemonic`. Verbose mode (`RUST_LOG=ncsync=trace,ncsync_unsafe=1`) suppresses redaction for local debugging only.

---

## Observability

### Log format

```json
{"ts":"2026-05-29T14:21:09.412Z","level":"INFO","target":"ncsync_core::engine",
 "span":"sync_cycle","account":"alice@cloud.example","event":"upload_complete",
 "path":"work/report.docx","bytes":204812,"duration_ms":1183,"chunks":1}
```

Stable event names (subset): `sync_cycle_start`, `sync_cycle_end`, `propfind`, `upload_start`, `upload_complete`, `upload_error`, `download_start`, `download_complete`, `download_error`, `conflict_detected`, `conflict_resolved`, `vfs_hydrate_start`, `vfs_hydrate_end`, `e2ee_unlock`, `peer_transfer_used`, `peer_fallback`, `throttle_pause`, `throttle_resume`.

### Metrics endpoint

The daemon exposes `127.0.0.1:<auto-port>/metrics` in Prometheus text format **only when `ncsync.toml` sets `metrics.enabled = true`**. Off by default. Useful for users self-hosting Grafana on the same machine.

### Bug-report bundle

`ncsync bug-report` produces `ncsync-report-YYYYMMDD-HHMM.tar.gz` containing:

- last 7 days of rotated logs (redacted),
- `ncsync status --json` output,
- redacted copy of `ncsync.toml`,
- system info (`uname -a` / `systeminfo` / `system_profiler` summary),
- daemon version, server version (from cached capabilities),
- a one-page TEMPLATE.md the user fills in.

The bundle is never auto-uploaded; the user attaches it manually to whichever issue tracker.

---

## Performance Engineering

### Budgets (mapped to spec NFRs)

| Metric                                          | Budget          |
|-------------------------------------------------|-----------------|
| RSS idle, 100k files tracked                    | < 150 MB        |
| RSS daemon-only headless                        | < 60 MB         |
| CPU idle, 5-min avg, sync complete              | < 1%            |
| RSS growth over 24 h idle                       | < 20%           |
| Cold start to first sync action                 | < 3 s           |
| Latency: local change to server visible (<10MB) | < 5 s           |
| Worst-case retry loop                           | ≤ 3 req/s/path  |

### Engineering levers

- Stream PROPFIND XML through `quick-xml` reader; never materialise the whole document.
- Database access exclusively through prepared statements; reuse one transaction per sync cycle for batched updates.
- Avoid `String` in hot paths; use `&str` and `Cow<'_, str>` for paths and ETags.
- Use BLAKE3 (vectorised) for local hashing; mmap files > 1 MB.
- No timers under 1 s in the daemon; the only fast loop is the filesystem-watcher channel reader.
- `cargo bloat` and `cargo-llvm-lines` gated in CI; flag any single function > 50k LLVM lines.

### Benchmark suite

`cargo bench` with `criterion`:

- WebDAV scan throughput vs a synthetic server (`mockito`) with 10k, 100k, 1M items.
- Reconciliation diff on synthetic state matrices.
- Chunked upload with simulated 200 ms RTT, 5% packet loss.
- E2EE encrypt/decrypt throughput per CPU core.

Reference machine for budgets: 4-core x86_64, 16 GB RAM, NVMe SSD; ARM reference: Raspberry Pi 5 (8 GB), microSD UHS-I.

---

## Packaging & Distribution

| OS           | Format                          | Notes |
|--------------|----------------------------------|-------|
| Windows      | MSI via WiX 4                    | Signed with EV cert; per-user install default |
| macOS        | DMG with notarized `.app` + helper LaunchAgent | Sparkle for updates |
| Debian/Ubuntu| `.deb`                           | systemd user service `ncsyncd.service` |
| Fedora/RHEL  | `.rpm`                           | same |
| Universal    | Flatpak                          | `org.ncsync.Client` on Flathub |
| Universal    | AppImage                         | for distros without Flatpak |
| Headless     | Static musl tarball              | `ncsyncd` + `ncsync` only, no GUI |

Auto-update is **opt-in**, per spec. The default release cadence is monthly point releases; security patches are out-of-band with a clearly different version suffix.

Sources of truth for builds:
- A reproducible build pipeline (`cargo build --locked --profile dist`) gated on `RUSTC_BOOTSTRAP=0` and a pinned toolchain (`rust-toolchain.toml`).
- All third-party Rust dependencies vendored in CI for offline builds and SBOM generation.

---

## Testing Strategy

### Test pyramid

1. **Unit tests** — every module, in-source `#[cfg(test)]`.
2. **Contract tests** — generated from `proto/ncsync.proto` and from a pinned snapshot of the Nextcloud OpenAPI documents. Any drift fails CI.
3. **Integration tests** — `testcontainers-rs` spins up `nextcloud/all-in-one` per test class. The suite covers:
   - First-run scan correctness against a seeded server tree.
   - Round-trip with all conflict matrix cells.
   - Chunked upload resume after kill -9.
   - E2EE end-to-end with two simulated devices (two daemon instances on the same host).
   - VFS hydration on Linux (Windows and macOS VFS in self-hosted runners).
   - Bandwidth cap accuracy with `tc netem` shaping.
4. **Soak tests** — 24-hour run with synthetic activity, asserting memory budget.
5. **Fuzzing** — `cargo-fuzz` targets for PROPFIND XML parsing, E2EE metadata, exclude-pattern matcher.

### Coverage gate

`cargo-llvm-cov` ≥ 80% lines on `ncsync-core`; ≥ 60% on platform-specific VFS code (which is harder to unit-test).

### Self-hosted CI fleet

- Linux x86_64 and aarch64 runners on a small bare-metal pool.
- Windows 11 and macOS Sonoma runners for VFS tests.
- Each runner has a dedicated Nextcloud AIO instance reset between runs.

---

## Quickstart Validation

After implementation, the spec is considered functionally satisfied when the following sequence succeeds end-to-end:

```bash
# 1. Install (Linux example)
sudo apt install ./ncsync_4.0_amd64.deb
systemctl --user enable --now ncsyncd.service

# 2. Add an account
ncsync account add \
    --server https://cloud.example.com \
    --user alice

# (browser opens for OAuth login flow v2; app password stored in keychain)

# 3. Verify resource budget
ncsync status --json | jq '.process.rss_bytes'           # expect < 150 MB
top -b -n 1 -p $(pgrep ncsyncd) | tail -1                # expect ~0% CPU after idle

# 4. Sync a 10 GB tree
mkdir -p ~/Nextcloud/big
dd if=/dev/urandom of=~/Nextcloud/big/blob.bin bs=1M count=10240
ncsync sync                                              # blocks until complete

# 5. Exercise excludes with negation
ncsync exclude add ".git/"
ncsync exclude add "!.git/config"
git init ~/Nextcloud/big/repo
ncsync sync
# .git is excluded, .git/config is present on server

# 6. Conflict
echo "local"  > ~/Nextcloud/big/notes.txt
# simulate remote change via curl/PUT, then:
ncsync sync
ncsync conflict list --json
ncsync conflict resolve big/notes.txt --keep both

# 7. E2EE
ncsync e2ee init             # generates mnemonic, requires 3-word confirmation
mkdir ~/Nextcloud/secret
ncsync e2ee encrypt secret
echo "private" > ~/Nextcloud/secret/file.txt
ncsync sync

# 8. CLI smoke
ncsync log tail --follow &
ncsync pause; ncsync resume
ncsync bug-report
```

If every step above completes with exit code `0` and the recorded resource budgets, the milestone closes.

---

## Risks & Open Technical Questions

| # | Risk / question | Mitigation / decision needed |
|---|-----------------|------------------------------|
| R1 | macOS File Provider replicated extensions have a steep learning curve and Apple sometimes regresses the API across OS minor versions. | Build a fallback path through `osxfuse` / `macFUSE` behind an `--unsupported-vfs` flag; track parity in CI matrix. |
| R2 | Tauri v2 webview availability on older Linux distros (e.g. CentOS 7 derivatives). | Ship the GUI as Flatpak with a bundled webview runtime; the CLI/daemon `.deb`/`.rpm` work everywhere. |
| R3 | Nextcloud E2EE RFC version drift; the metadata format changed twice in 2024–2025. | Treat the `metadata_ver` field as a hard gate; refuse writes when the server advertises a version newer than the client knows. |
| R4 | LAN-peer protocol could leak metadata to other devices on hostile networks. | Off by default; require explicit per-account enable; mDNS only on user-allowlisted interfaces. |
| R5 | Bulk-upload endpoint behaviour with E2EE is undocumented. | Disable bulk-upload for E2EE folders in v1.0; revisit when the server-side behaviour is specified. |
| Q1 | RSA-2048 vs P-256 for the user key pair — server enforces RSA today. | Track upstream RFC 2.2; ship RSA in v1.0, add P-256 behind a feature flag the moment server side allows. |
| Q2 | Should the daemon expose its IPC over a TCP loopback port in addition to UDS/pipe? | Default no; opt-in for users on Windows who run the daemon as a service under a different account. Decide at `/speckit.clarify`. |
| Q3 | FreeBSD parity. | Out of v1.0 unless a contributor steps in; the Rust stack is portable, but no CI runner is provisioned. |
| Q4 | Should we ship a Prometheus exporter or only structured logs? | Logs in v1.0; metrics behind a feature flag in v1.1. |
| Q5 | Snap packaging. | Skip; Flatpak + AppImage cover the same surface without confining the FUSE mount path. |

---

## Phase Plan

### Phase 0 — Research & spikes (2 weeks)

- W1: WebDAV client spike against a live AIO container; reproduce known issues from spec (busy/locked files, runaway loop) and capture telemetry.
- W2: VFS spike on each OS; confirm `CfApi`, `NSFileProviderReplicatedExtension`, and FUSE3 can be driven from Rust with the chosen crates.

Exit criteria: a one-page write-up per spike with a go/no-go on each platform.

### Phase 1 — Foundations (6 weeks)

- `ncsync-core` skeleton, DB schema, account add, WebDAV client, scan + diff, file CRUD without VFS.
- `ncsync` CLI minimal command set (`account`, `sync`, `status`, `log`).
- CI: contract tests, integration tests against AIO.

Exit criteria: round-trip of a 1 GB folder against a live server with all conflict matrix cells covered.

### Phase 2 — Daemon & GUI (6 weeks)

- `ncsyncd` long-running process, IPC, settings, multi-account.
- Tauri shell with account list, sync status, conflict UI, settings panels.
- Bandwidth & network awareness.

Exit criteria: a tester can install, configure two accounts, and run for a week without intervention.

### Phase 3 — VFS (8 weeks)

- Windows `CfApi`.
- macOS File Provider.
- Linux FUSE3.

Exit criteria: hydration latency budget met on all three platforms.

### Phase 4 — E2EE, LAN-peer, polish (6 weeks)

- E2EE init, encrypt/decrypt, QR pairing, metadata 2.x compliance.
- LAN-peer discovery and transfer.
- Bug-report bundle, full CLI, package builds for all targets.

Exit criteria: the entire quickstart passes on a fresh VM for each OS.

### Phase 5 — Hardening & release (4 weeks)

- Soak tests, fuzzing, security review, third-party dependency audit (`cargo-audit`, `cargo-deny`).
- Documentation site (`docs.ncsync.dev`).
- v1.0 release.

Total: 32 weeks to v1.0 with a team of two; 20 weeks at three.

---

## Traceability Matrix (excerpt)

| Spec FR | Plan section                              |
|---------|-------------------------------------------|
| FR-01   | Sync Engine Algorithm; WebDAV Transport   |
| FR-02   | Virtual Files Per Platform                |
| FR-03   | Data Model (`excludes` table); Conflict §  |
| FR-04   | Conflict Resolution §                      |
| FR-05   | Component Design (per-account daemon tasks)|
| FR-06   | Network & Bandwidth Layer                  |
| FR-07   | Component Design (CLI); IPC               |
| FR-08   | `share` module; WebDAV Transport § (OCS)  |
| FR-09   | End-to-End Encryption Implementation §    |
| FR-10   | (Phase 4 +) deferred local index module   |
| FR-11   | LAN-Peer Protocol §                        |
| FR-12   | (Phase 4 +) mount-mode driver via FUSE3   |
| FR-13   | Observability §; bug-report bundle         |
| FR-14   | Security & Credential Storage §            |