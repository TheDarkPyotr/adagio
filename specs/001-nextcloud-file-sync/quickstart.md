# Quickstart: Adagio Development Setup

**Feature**: 001-nextcloud-file-sync
**Updated**: 2026-05-24

This guide gets a developer from zero to running tests and the desktop app locally.

---

## Prerequisites

| Tool | Version | Install |
|------|---------|---------|
| Rust (stable) | ≥ 1.78 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js | ≥ 20 LTS | via `nvm` or `fnm` |
| Tauri CLI | 2.x | `cargo install tauri-cli --version "^2"` |
| cargo-sqlx | 0.8.x | `cargo install sqlx-cli --features sqlite` |
| Docker | any | For integration tests (Nextcloud container) |
| SQLite | 3.x | system package (`libsqlite3-dev` / `sqlite3`) |

**Platform extras**:
- **Linux**: `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `libayatana-appindicator3-dev`
  (system tray), `libsecret-1-dev` (keyring)
- **macOS**: Xcode Command Line Tools (`xcode-select --install`)
- **Windows**: Microsoft Edge WebView2 Runtime (ships with Windows 11; installer at
  microsoft.com/edge/webview2 for Windows 10)

---

## Clone and Build

```bash
git clone https://github.com/TheDarkPyotr/adagio.git
cd adagio

# Install Node dependencies for the Svelte frontend
cd crates/adagio-desktop/src-ui && npm install && cd -

# Run SQLx migrations to prepare the development SQLite schema
export DATABASE_URL="sqlite://dev-journal.db"
cargo sqlx database create
cargo sqlx migrate run --source crates/adagio-core/migrations

# Build all workspace crates
cargo build --workspace
```

---

## Run Unit Tests

```bash
# All crates, excluding integration tests
cargo test --workspace --exclude adagio-desktop
```

Unit tests run in-process with an in-memory `Journal` implementation and a mock
`RemoteClient`. No network access or Docker required.

---

## Run Contract Tests

```bash
cargo test -p adagio-core --test journal_contract --test remote_client_contract
```

Contract tests verify that the SQLite `JournalStore` and the Nextcloud `WebDavClient`
satisfy the `Journal` and `RemoteClient` trait contracts. They require the SQLite DB
(set up above) but no network access.

---

## Run Integration Tests

### Mock-server tests (no Docker required)

The error-recovery and conflict tests use an in-process mock client. They are marked
`#[ignore]` to keep the default `cargo test` run fast, so pass `--ignored` explicitly:

```bash
cargo test -p adagio-core --test error_recovery --test conflict_integration -- --ignored
```

### Live-server tests (Docker required)

The sync-cycle and transfer tests exercise the full stack against a real Nextcloud
instance. Start the server first, then run:

```bash
# Start Nextcloud in Docker (username: admin, password: admin)
docker run -d \
  --name adagio-nextcloud \
  -p 8080:80 \
  -e NEXTCLOUD_ADMIN_USER=admin \
  -e NEXTCLOUD_ADMIN_PASSWORD=admin \
  nextcloud:28

# Wait ~30 s for Nextcloud to initialize, then run live integration tests
ADAGIO_TEST_SERVER=http://localhost:8080 \
ADAGIO_TEST_USER=admin \
ADAGIO_TEST_PASSWORD=admin \
cargo test -p adagio-nextcloud -- --ignored
```

Integration tests clean up all test data on the server after each run.

---

## Run the Desktop App (Development Mode)

```bash
# Starts Tauri in dev mode with HMR for the Svelte frontend
cd crates/adagio-desktop && cargo tauri dev
```

The dev build connects to a real Nextcloud server. Configure an account in the app's
onboarding screen, pointing to your local Docker instance or any Nextcloud server you
have access to.

---

## Run Benchmarks

```bash
# Reconciler hot path and SHA-256 throughput
cargo criterion
```

Results are written to `target/criterion/`. Compare against the baseline before
any changes that touch `crates/adagio-core/src/cycle/reconciler.rs` or
`crates/adagio-core/src/transfer/`.

---

## Linting and Formatting

All of these must pass before a PR is opened (they are CI gates):

```bash
cargo fmt --check
# Exclude adagio-desktop on Linux: it requires GTK headers not present on headless systems
cargo clippy --workspace --exclude adagio-desktop -- -D warnings
cargo doc --workspace --no-deps --exclude adagio-desktop
```

---

## Database Migrations

SQLx migrations live in `crates/adagio-core/migrations/`. To add a migration:

```bash
cargo sqlx migrate add <migration-name>
# Edit the generated file in crates/adagio-core/migrations/
cargo sqlx migrate run --source crates/adagio-core/migrations
```

After adding migrations, regenerate the offline SQLx query cache:

```bash
cargo sqlx prepare --workspace
```

Commit both the migration file and the updated `.sqlx/` query cache.

---

## Architecture Quick Reference

```
adagio-desktop (Tauri shell)
      │ Tauri IPC commands
      ▼
adagio-core (sync engine)
      │ RemoteClient trait
      ▼
adagio-nextcloud (WebDAV + NC protocol)
      │ HTTPS
      ▼
Nextcloud server
```

- All business logic lives in `adagio-core`. Never import `adagio-desktop` types from
  `adagio-core`.
- `adagio-nextcloud` depends on `adagio-core` traits only (not the implementations).
- `adagio-desktop` depends on both `adagio-core` and `adagio-nextcloud`.

---

## Common Development Tasks

| Task | Command |
|------|---------|
| Add a new dependency | `cargo add <crate> -p adagio-core` |
| Check compile errors fast | `cargo check --workspace` |
| Run a single test | `cargo test -p adagio-core <test_name>` |
| View logs in dev mode | Set `RUST_LOG=adagio=debug` before `cargo tauri dev` |
| Inspect journal DB | `sqlite3 dev-journal.db` |
| Rebuild Svelte UI only | `cd crates/adagio-desktop/src-ui && npm run build` |
