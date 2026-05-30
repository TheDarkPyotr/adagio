# Quickstart Validation: Bulk Upload Driver (011)

End-to-end validation for all five user stories.

---

## Prerequisites

```bash
cargo build -p adagio-daemon -p adagio-cli

# Create a test folder with many small files
mkdir -p /tmp/bulk-test
for i in $(seq 1 100); do dd if=/dev/urandom of=/tmp/bulk-test/file-$i.bin bs=1024 count=100 2>/dev/null; done
# 100 files × 100 KB = 10 MB total

# Start daemon
./target/debug/adagio-daemon --config-dir ~/.config/ai.neuralagent.adagio/ &
sleep 1
```

---

## US1 — Parallel uploads faster than sequential

```bash
# Add a sync pair pointing at /tmp/bulk-test (remote must be empty first)
./target/debug/adagio accounts list  # note account_id
# Add pair via UI or CLI

# Trigger sync and measure time
time ./target/debug/adagio sync

# Verify all files uploaded
./target/debug/adagio status --json | python3 -m json.tool
# Expected: status=idle, no errors

# Check journal
# Expected: 100 journal entries with status=synced
```

**Pass criteria:** Wall-clock time < 8× single sequential upload time for 10 MB. All 100 files on remote.

---

## US2 — Resume after interruption

```bash
# Start a sync, then kill the daemon mid-upload (use a larger folder for this)
mkdir -p /tmp/bulk-resume
for i in $(seq 1 200); do dd if=/dev/urandom of=/tmp/bulk-resume/file-$i.bin bs=1024 count=500 2>/dev/null; done
# 200 files × 500 KB = 100 MB

# Start sync
./target/debug/adagio sync &
SYNC_PID=$!

# Kill after ~5 seconds
sleep 5 && kill $SYNC_PID

# Count uploaded files in journal
# (using sqlite3 or the daemon)
./target/debug/adagio status  # note files completed

# Restart daemon and sync again
./target/debug/adagio-daemon --config-dir ~/.config/ai.neuralagent.adagio/ &
sleep 1
./target/debug/adagio sync

# Verify ONLY remaining files are uploaded (not the already-completed ones)
# Check daemon logs for upload count — should be < 200
```

**Pass criteria:** Second sync uploads fewer files than total; sum of both runs = 200.

---

## US3 — Chunked upload for large files

```bash
# Create a file > 10 MB
dd if=/dev/urandom of=/tmp/bulk-test/large.bin bs=1M count=25

# Trigger sync
./target/debug/adagio sync

# Check daemon logs for "upload_chunked" vs "upload_single"
# Expected: large.bin uses chunked protocol

# Test chunk resume by interrupting mid-upload of large.bin
# Kill daemon during upload, restart, sync again
# Check that only remaining chunks are sent (log shows fewer bytes than full file)
```

**Pass criteria:** Files ≥ 10 MB use chunked protocol. Interrupted chunked upload resumes from last committed chunk.

---

## US4 — Progress visible in UI and CLI

```bash
# Start bulk upload of large folder
./target/debug/adagio sync &

# While running, check status every second
for i in $(seq 1 10); do
    ./target/debug/adagio status
    sleep 1
done

# Expected: status shows active file count increasing, bytes transferred increasing
```

Open desktop app while bulk upload is running:
1. Check sync status indicator shows "Syncing X files"
2. Activity log shows completed uploads in real time
3. Status updates at least once per second

**Pass criteria:** File count and byte count advance visibly during upload.

---

## US5 — Automatic activation

```bash
# Create fresh pair with empty remote
# Ensure /tmp/bulk-test has ≥ 50 files and remote is empty

# Run sync — should auto-activate bulk driver
./target/debug/adagio sync

# Check daemon logs for "bulk upload activated" or similar message
# After completion, standard cycle should run for any changed files
```

**Pass criteria:** Bulk driver activates without any special flags. Standard cycle takes over after completion.

---

## Quality gates

```bash
cargo clippy -- -D warnings
cargo fmt --all --check
cargo test --lib -p adagio-core     # includes bulk driver unit tests
cargo test -p adagio-daemon
npm run test                         # UI tests (no changes expected)
```
