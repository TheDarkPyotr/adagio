# Quickstart Validation: CLI Binary (008)

End-to-end validation for all five user stories.

---

## Prerequisites

```bash
# Build both binaries (daemon + CLI)
cargo build -p adagio-daemon -p adagio-cli 2>&1 | grep -c "^error"  # expect 0

# Confirm binaries exist
ls -la target/debug/adagio target/debug/adagio-daemon

# Start the daemon (or let CLI auto-start it)
pkill adagio-daemon 2>/dev/null; true
```

---

## US1 — Check sync status (P1)

```bash
# With daemon already running
./target/debug/adagio-daemon --config-dir ~/.config/adagio/ &
sleep 1

./target/debug/adagio status
# Expect: table with pair ID, local path, status (idle), last synced

./target/debug/adagio status --json | python3 -m json.tool
# Expect: valid JSON array, zero parse errors

echo "Exit code: $?"  # expect 0
```

**Pass criteria**: Table shows all configured pairs; `--json` produces parseable JSON; both exit 0.

---

## US2 — Trigger sync / auto-start (P2)

```bash
# Test auto-start: kill daemon, then sync
pkill adagio-daemon 2>/dev/null; sleep 1

time ./target/debug/adagio sync
# Expect: daemon starts, sync triggers, command exits 0 within 8 seconds

echo "Exit: $?"  # expect 0

# Test with specific pair ID
PAIR_ID=$(./target/debug/adagio pairs list --json | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])")
./target/debug/adagio sync "$PAIR_ID"
echo "Exit: $?"  # expect 0
```

**Pass criteria**: `adagio sync` with no daemon running completes in under 8 seconds; exits 0.

---

## US3 — Manage conflicts (P3)

```bash
# Create a conflict: modify same file on both local and server, trigger sync
echo "local" > ~/adagio-test/conflict-test.txt
# [modify the same file on server via Nextcloud web UI]
./target/debug/adagio sync

./target/debug/adagio conflicts list
# Expect: conflict-test.txt shown with local/server metadata

CONFLICT_ID=$(./target/debug/adagio conflicts list --json | python3 -c "import sys,json; print(json.load(sys.stdin)[0]['id'])")

./target/debug/adagio conflicts resolve "$CONFLICT_ID" --keep local
echo "Exit: $?"  # expect 0

./target/debug/adagio conflicts list
# Expect: empty list (conflict resolved)

# Test dismiss
# [create another conflict]
./target/debug/adagio conflicts dismiss
echo "Exit: $?"  # expect 0
```

**Pass criteria**: Conflicts list shows the conflict; resolve exits 0; list is empty after resolution.

---

## US4 — JSON output contract (P4)

```bash
# Every command that produces output must return valid JSON with --json
commands=(
  "status"
  "pairs list"
  "accounts list"
  "conflicts list"
  "daemon status"
  "activity"
)

ALL_PASS=true
for cmd in "${commands[@]}"; do
  output=$(./target/debug/adagio $cmd --json 2>/dev/null)
  if echo "$output" | python3 -m json.tool > /dev/null 2>&1; then
    echo "PASS: adagio $cmd --json"
  else
    echo "FAIL: adagio $cmd --json — not valid JSON: $output"
    ALL_PASS=false
  fi
done

$ALL_PASS && echo "All JSON output tests pass"

# Verify no JSON on stdout when command fails (daemon stopped)
pkill adagio-daemon
./target/debug/adagio status --json 2>/dev/null
echo "Exit: $?"  # expect 3
```

**Pass criteria**: All listed commands produce valid JSON with `--json`; exit code 3 when daemon unreachable.

---

## US5 — Daemon lifecycle (P5)

```bash
# Stop + start cycle
pkill adagio-daemon 2>/dev/null; sleep 1

time ./target/debug/adagio daemon start
echo "Exit: $?"  # expect 0, within 5 seconds

./target/debug/adagio daemon status
# Expect: running=true, uptime in seconds, version

./target/debug/adagio daemon status --json | python3 -c "
import sys, json
d = json.load(sys.stdin)
assert d['running'] == True
print('daemon status JSON OK')
"

# Double-start is idempotent
./target/debug/adagio daemon start
echo "Exit: $?"  # expect 0 (already running)

# Stop
./target/debug/adagio daemon stop
echo "Exit: $?"  # expect 0

pgrep adagio-daemon && echo "FAIL: daemon still running" || echo "PASS: daemon stopped"
```

**Pass criteria**: `daemon start` completes in < 5s; `daemon status` shows running; `daemon stop` stops it; exit codes all 0.

---

## Quality gates

```bash
cargo clippy -p adagio-cli -- -D warnings   # 0 warnings
cargo fmt --all --check                     # passes
cargo test --lib -p adagio-cli              # all tests pass
cargo test -p adagio-daemon                 # still 6 pass (no regressions)
cargo test --lib -p adagio-ipc              # still 13 pass
```

---

## Pass criteria (all 5 stories)

- [ ] US1: `adagio status` prints pairs table; `--json` outputs valid JSON
- [ ] US2: `adagio sync` auto-starts daemon if needed, completes in < 8s
- [ ] US3: Full conflict workflow (list → resolve) works end-to-end
- [ ] US4: All output commands produce valid JSON with `--json`; correct exit codes
- [ ] US5: `daemon start/stop/status` all work; start in < 5s
