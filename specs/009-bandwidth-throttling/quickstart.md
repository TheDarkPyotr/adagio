# Quickstart Validation: Bandwidth Throttling (009)

End-to-end validation for all five user stories.

---

## Prerequisites

```bash
# Build everything
cargo build -p adagio-core -p adagio-daemon -p adagio-cli 2>&1 | grep -c "^error"  # 0
cargo test --lib -p adagio-core  # still 133+ pass

# Daemon running with test config
./target/debug/adagio-daemon --config-dir ~/.config/adagio/ &
sleep 1
```

---

## US1 + US2 — Upload and download throttling (P1+P2)

```bash
# Set a 200 Kbps upload limit
./target/debug/adagio bandwidth set --upload-kbps 200
echo "Exit: $?"  # 0

# Trigger a sync that uploads at least 2 MB
./target/debug/adagio sync

# Measure: verify the upload did not exceed ~204 800 bytes/sec on average.
# Rough check: a 2 MB upload at 200 Kbps should take at least 8 seconds.
# Time it:
time ./target/debug/adagio sync
# Expected: real time ≥ 8s for a 2 MB upload under 200 Kbps limit

# Set download limit, leave upload unlimited
./target/debug/adagio bandwidth set --download-kbps 500
./target/debug/adagio bandwidth status
# Expect: Upload = Unlimited (or previous value), Download = 500 Kbps

# Verify upload is NOT throttled (remove upload limit first)
./target/debug/adagio bandwidth set --upload-kbps 0
# Both directions now: upload=unlimited, download=500 Kbps

# Clear all
./target/debug/adagio bandwidth clear
./target/debug/adagio bandwidth status --json | python3 -m json.tool
# Expect: upload_limit_kbps=0, download_limit_kbps=0
```

**Pass criteria**: Upload takes ≥ expected time under limit; limits are independent.

---

## US3 — Settings panel (P3)

```bash
# Start the desktop app
cd crates/adagio-desktop && npm run tauri:dev &

# In the app:
# 1. Open Settings → Sync → Bandwidth
# 2. Enter 300 in the upload limit field, leave download blank
# 3. Click Save
# 4. Verify the field shows 300 after saving
# 5. Trigger a sync — verify upload is throttled to ~300 Kbps
# 6. Close and reopen the app
# 7. Verify the 300 Kbps limit is still shown and enforced

# Test validation: enter "abc" in the upload field
# Expect: field turns red, save is blocked
```

**Pass criteria**: Limits persist across app restarts; invalid input is rejected.

---

## US4 — CLI (P4)

```bash
# Full CLI workflow
./target/debug/adagio bandwidth status
# Expect: both directions show "Unlimited"

./target/debug/adagio bandwidth set --upload-kbps 100 --download-kbps 200
./target/debug/adagio bandwidth status
# Expect: Upload = 100 Kbps, Download = 200 Kbps

./target/debug/adagio bandwidth set --upload-kbps 150
./target/debug/adagio bandwidth status
# Expect: Upload = 150 Kbps, Download = 200 Kbps (only upload changed)

./target/debug/adagio bandwidth clear
./target/debug/adagio bandwidth status --json
# Expect: {"upload_limit_kbps":0,"download_limit_kbps":0,...}

# Test with daemon stopped
pkill adagio-daemon
./target/debug/adagio bandwidth set --upload-kbps 50
echo "Exit: $?"  # 0 (daemon auto-started)
```

**Pass criteria**: All commands exit 0; status reflects changes; auto-start works.

---

## US5 — Live throughput display (P5)

```bash
# Start a large upload (>5 MB) with a 200 Kbps limit
./target/debug/adagio bandwidth set --upload-kbps 200
./target/debug/adagio sync &

# While sync is running, check status several times:
sleep 2; ./target/debug/adagio bandwidth status
sleep 2; ./target/debug/adagio bandwidth status
sleep 2; ./target/debug/adagio bandwidth status
# Expect: upload_bytes_per_sec shows a non-zero value ≤ ~25600 (200 Kbps in bytes/s)
# Expect: values update between readings

# When sync finishes:
./target/debug/adagio bandwidth status
# Expect: upload_bytes_per_sec = 0 (or near 0)
```

**Pass criteria**: Live speed is non-zero during active transfer; updates within 2s.

---

## Quality gates

```bash
cargo clippy -- -D warnings              # 0 errors
cargo fmt --all --check                  # passes
cargo test --lib -p adagio-core          # 133+ pass (including new bandwidth tests)
cargo test --lib -p adagio-ipc           # 13+ pass
cargo test -p adagio-daemon              # 6+ pass
cargo test --lib -p adagio-desktop       # 41+ pass
npm run test                             # 95+ pass
```

---

## Accuracy benchmark (automated)

```bash
cargo test -p adagio-core -- bandwidth_throttle_accuracy
# The test uploads a 2 MB synthetic payload against a mock client,
# measures elapsed time, and asserts:
#   elapsed ≥ expected_minimum (within 10% tolerance)
```
