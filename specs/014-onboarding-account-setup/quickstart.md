# Quickstart: Onboarding — Account Setup Wizard (Dev Setup)

**Feature**: 014-onboarding-account-setup | **Date**: 2026-06-01

## Prerequisites

- Rust 1.92+ (stable; `rustup update stable`)
- Node.js 20+ with npm
- GTK3 dev libraries on Linux (see below)
- A running Nextcloud instance (local Docker or remote)

### Linux — required system libraries

```bash
sudo apt-get install -y \
  libgtk-3-dev libgdk-pixbuf2.0-dev libcairo2-dev \
  libpango1.0-dev libatk1.0-dev libsoup-3.0-dev \
  libwebkit2gtk-4.1-dev
```

## Running the Desktop App in Dev Mode

```bash
# 1. Install frontend dependencies (first time only)
cd crates/adagio-desktop/src-ui
npm install
cd ../../..

# 2. Start the Tauri dev server (hot-reload UI + auto-rebuild Rust on change)
cd crates/adagio-desktop
npm run tauri:dev
```

## Force Onboarding to Show

By default the wizard only shows when no accounts are saved. To test the wizard
without removing your real account, flip the dev flag in `App.tsx`:

```typescript
// crates/adagio-desktop/src-ui/src/App.tsx  (line ~28)
const DEV_FORCE_ONBOARD = true;  // ← flip this
```

Revert before committing.

## Testing with a Local Nextcloud

```bash
# Start Nextcloud in Docker (from repo root)
docker-compose up -d nextcloud

# Apply E2EE server patches if needed
docker cp /tmp/patch_e2ee.php nextcloud:/tmp/patch_e2ee.php
docker exec nextcloud php /tmp/patch_e2ee.php

# Default credentials: admin / admin
# Default URL: http://localhost:8080
```

Use `http://localhost:8080` as the server URL in step 2 of the wizard.
The Login Flow v2 endpoint is at `http://localhost:8080/index.php/login/v2`.

## Running Backend Tests

```bash
# All unit + integration tests for the desktop crate
cargo test -p adagio-desktop

# Onboarding-specific tests only
cargo test -p adagio-desktop onboarding

# Nextcloud login flow protocol tests
cargo test -p adagio-nextcloud login_flow
```

## Running Frontend Tests

```bash
cd crates/adagio-desktop/src-ui
npm test          # vitest, watch mode
npm run test:run  # single pass (for CI)
```

## Verifying End-to-End Acceptance

After wiring all 5 steps, verify each acceptance criterion manually:

| Step | What to verify |
|------|---------------|
| 2 — Server | Enter a valid URL → badge shows real version within 3 s. Enter a bad URL → inline error, Continue blocked. |
| 3 — Authorize | Real code displayed (not "F4PS · 9TRX"). QR scannable with phone → opens NC login. Countdown ticks. After browser auth → wizard advances automatically (no click). |
| 4 — Folder | "Browse…" opens OS picker. Chosen path shown in input. Toggle states survive Back → Continue navigation. |
| 5 — Begin | File count + storage match NC web UI (Settings → Personal info). Progress bar moves as files sync. "Open Adagio" → main view. |
| Restart | Quit app. Reopen. Toggle states match what was set in step 4. |

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `RUST_LOG=adagio_desktop=debug` | Enable debug logs for all desktop commands |
| `RUST_LOG=adagio_nextcloud=debug` | Log Login Flow v2 poll requests and responses |
| `ADAGIO_CONFIG_DIR=/tmp/adagio-test` | Use a throw-away config directory for isolated testing |
