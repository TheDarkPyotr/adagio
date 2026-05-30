# T052 Validation: Sync Resource Efficiency (006)

Manual and semi-automated validation for all four acceptance criteria.

---

## Prerequisites

```bash
LOCAL="$HOME/Adagio"            # your pair local root
PAIR_ID="<uuid>"                # from Settings → Sync Pairs

cargo test --lib -p adagio-core # all tests pass before validation
```

---

## AC-1 — CPU ≤ 1% (5-min average) after 60 s idle

### Automated benchmark

```bash
cargo bench -p adagio-core -- idle_scan
```

Expected: bench shows < 2 ms/iter for 1,000-file unchanged directory.
Also verify in bench output that `checksum_recomputed = 0` (log field).

### Manual spot-check

1. Sync any folder completely (no pending changes).
2. Leave the app running for 60 seconds with no local or remote changes.
3. Measure CPU:

```bash
# Linux
top -b -n 30 -d 10 -p $(pgrep adagio) | grep adagio | awk '{sum+=$9} END {print sum/30 "%"}'

# macOS
ps -p $(pgrep adagio) -o %cpu | tail -1   # sample several times
```

**Pass**: Average < 1.0%.

### Verify `MissedTickBehavior::Skip` is in effect

```bash
grep -n "set_missed_tick_behavior" crates/adagio-core/src/cycle/runner.rs
# Expected: one hit on the line after interval() construction
```

---

## AC-2 — No file locks visible to other applications

### Automated test

```bash
cargo test -p adagio-core -- scan_concurrent_open_file_no_lock
```

This test opens a file with `std::fs::File::open` (simulating an editor),
runs the scanner concurrently, and asserts the open call does not receive
`ErrorKind::PermissionDenied` or `WouldBlock`.

### Manual spot-check (Linux)

1. Open a file in your synced folder with an editor (e.g., `vim $LOCAL/notes.txt`).
2. Trigger a sync cycle from Settings → Sync Pairs.
3. While the cycle runs, switch to the editor — file must be readable and writable.

```bash
# Verify scanner holds no fd on the file after the cycle completes:
lsof -p $(pgrep adagio) | grep notes.txt
# Expected: no output (no open handle)
```

### Manual spot-check (Windows)

Run `handle64.exe -p adagio.exe` during an active scan and confirm no exclusive
file handles appear for files in the sync folder.

---

## AC-3 — RSS growth ≤ 20% in 24 h

### Automated regression test

```bash
cargo test -p adagio-core -- memory_stable_over_many_cycles
```

This test runs 500 simulated cycles with a mock client and verifies that the
final RSS reading (from `MemorySampler::sample_rss()`) does not exceed 120% of
the initial reading.

### Manual 24-hour soak

1. Note baseline RSS 60 seconds after startup:

```bash
# Linux
cat /proc/$(pgrep adagio)/statm | awk '{print $1 * 4096}' # bytes
```

2. Run for 24 hours with a typical workload (mix of idle and occasional edits).

3. Check RSS again after 24 h:

```bash
cat /proc/$(pgrep adagio)/statm | awk '{print $1 * 4096}'
```

**Pass**: Final RSS ≤ 1.20 × baseline.

### Check WARN logs for early-warning triggers

```bash
journalctl -u adagio --since "24 hours ago" | grep '"rss_growth"' | grep WARN
# Ideally: no output; at most a few transient peaks
```

---

## AC-4 — ≤ 3 identical requests per second in retry loop

### Automated test

```bash
cargo test -p adagio-core -- retry_rate_never_exceeds_3_per_second
```

This test injects a mock client that always returns `TransientError`, records
timestamps of each call, and asserts no two calls on the same path are within
333 ms of each other (i.e., ≤3 req/s).

### Log audit

Run the app with `RUST_LOG=adagio_core=debug` for 30 minutes against a
server that occasionally returns 503:

```bash
RUST_LOG=adagio_core=debug cargo run -- 2>&1 | grep '"retry_attempt"'
```

For each path, verify:
- `delay_ms` in the log is always ≥ 1000 on the first retry.
- No two log entries for the same path appear within 1 s of each other.

---

## Quality gate checklist

```bash
# Rust
cargo clippy -- -D warnings        # 0 warnings
cargo fmt --all --check            # passes
cargo test --lib -p adagio-core    # all tests pass including new ones
cargo bench -p adagio-core -- idle_scan  # benchmark runs

# Verify no println! or eprintln! added
grep -rn 'println!\|eprintln!' crates/adagio-core/src/ crates/adagio-desktop/src/
# Expected: no output
```

---

## Pass criteria

T052 (this feature) passes when:
- [ ] All automated tests pass including the four new regression tests.
- [ ] Benchmark confirms `checksum_recomputed = 0` for unchanged folder.
- [ ] Manual CPU spot-check reads < 1% on your dev machine.
- [ ] Manual 24-h soak shows RSS growth < 20%.
- [ ] Log audit shows no `delay_ms < 1000` on first retry.
