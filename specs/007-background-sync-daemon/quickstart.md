# Quickstart Validation: Background Sync Daemon (007)

End-to-end validation for all five user stories. Requires a machine with a
configured sync pair.

---

## Automated gates (all pass — run before manual validation)

```bash
cargo clippy -- -D warnings          # 0 errors
cargo fmt --all --check              # clean
cargo test --lib -p adagio-core      # 133 pass
cargo test --lib -p adagio-ipc       # 13 pass
cargo test -p adagio-daemon          # 6 pass
cargo test --lib -p adagio-desktop   # 41 pass
npm run test                         # 95 pass
npm run check                        # 0 type errors

# Binaries are at:
ls target/debug/adagio-daemon        # standalone daemon
# adagio-desktop is Tauri — run via: npm run tauri:dev (from crates/adagio-desktop)
```

## Prerequisites

---

## US1 — Sync continues after window close

```bash
# 1. Start the desktop app
cd crates/adagio-desktop && npm run tauri:dev &

# 2. Confirm daemon started (new process, not embedded in Tauri)
pgrep -a adagio-daemon   # must print at least one PID

# 3. Note a sync pair's scan_interval or trigger a sync
# 4. Close the main window (window X button, NOT tray → Quit)

# 5. Confirm daemon is STILL running after window close
sleep 3
pgrep adagio-daemon     # PID must still exist

# 6. Make a remote change via Nextcloud web UI
# 7. Wait for next sync cycle or trigger via socket
echo '{"id":1,"method":"trigger_sync","params":{"pair_id":"<id>"}}' | \
  nc -U "$XDG_RUNTIME_DIR/adagio/daemon.sock"

# 8. Confirm file was downloaded even though window was closed
ls -la ~/Adagio/   # new/changed file should be present

# 9. Re-open the app — window should appear immediately with current status
```

**Pass criteria**: File synced while window was closed; daemon PID unchanged throughout.

---

## US2 — Auto-start at login

```bash
# 1. Enable "Start at login" from Settings in the app
# 2. Verify the auto-start entry was created:

# Linux:
cat ~/.config/autostart/adagio-daemon.desktop
# expect: Exec= pointing to adagio-daemon binary

# macOS:
cat ~/Library/LaunchAgents/com.adagio.daemon.plist
# expect: RunAtLoad = true; ProgramArguments = ["/path/to/adagio-daemon"]

# Windows:
reg query "HKCU\Software\Microsoft\Windows\CurrentVersion\Run" /v adagio-daemon
# expect: REG_SZ "C:\...\adagio-daemon.exe"

# 3. Kill all adagio processes and simulate re-login (or actually reboot)
pkill adagio-daemon; pkill adagio-desktop

# 4. Simulate login auto-start manually (without rebooting):
# Run whatever the auto-start entry specifies:
~/.config/autostart/adagio-daemon.desktop  # or parse Exec= and run it

# 5. Confirm daemon is running WITHOUT opening the GUI
sleep 5
pgrep adagio-daemon  # must be alive
# No GUI window should have appeared

# 6. Disable "Start at login" and confirm entry is removed:
# Linux:
ls ~/.config/autostart/adagio-daemon.desktop  # must NOT exist
```

**Pass criteria**: Daemon starts within 5 s of login; no window appears.

---

## US3 — Instant reconnect when app opens

```bash
# 1. Start the daemon standalone (without GUI)
./target/debug/adagio-daemon &
DAEMON_PID=$!
sleep 1  # wait for socket to be ready

# 2. Measure time from app launch to UI visible
time (npm run tauri:dev &)
# Check: status/pairs visible in under 1 second of window appearing

# 3. Open a second instance — should NOT start another daemon
./target/debug/adagio-desktop &
sleep 1
pgrep adagio-daemon | wc -l   # must be 1, not 2

# 4. Trigger a sync while app is open — confirm UI updates in real time
echo '{"id":1,"method":"trigger_sync","params":{"pair_id":"<id>"}}' | \
  nc -U "$XDG_RUNTIME_DIR/adagio/daemon.sock"
# UI should show "syncing" state without polling

kill $DAEMON_PID
```

**Pass criteria**: Window loads in < 1 s; only one daemon process; real-time updates.

---

## US4 — Start/stop from Settings

```bash
# 1. Open Settings in the app → click "Stop background sync"
# 2. Verify daemon stopped:
sleep 3
pgrep adagio-daemon  # must print nothing (or exit code 1)

# 3. Confirm app shows "Background sync is stopped" state

# 4. Close app window and reopen — stopped state must persist
# (no auto-restart on window close/open)

# 5. Click "Start background sync"
sleep 3
pgrep adagio-daemon  # must print a PID within 3 s

# 6. Verify sync resumes (trigger a sync and confirm activity log entry)
```

**Pass criteria**: Stop/start cycle completes; stopped state persists across window open/close.

---

## US5 — Crash recovery

```bash
# 1. Start daemon and open app
./target/debug/adagio-daemon &
DAEMON_PID=$!
# Open app and confirm connected

# 2. Kill the daemon externally (simulating a crash)
kill -9 $DAEMON_PID

# 3. Observe the app window:
#    - Within 3 s: "Reconnecting…" indicator must appear
#    - Within 10 s: app should automatically restart daemon and reconnect
sleep 12
pgrep adagio-daemon   # new PID must exist

# 4. Confirm sync resumes (trigger sync, check activity log)

# 5. Test failure path: prevent restart (make binary non-executable temporarily)
chmod -x ./target/debug/adagio-daemon
kill -9 $(pgrep adagio-daemon)
# After 3 restart attempts, app must show error message with "Restart sync" button
chmod +x ./target/debug/adagio-daemon
```

**Pass criteria**: Auto-restart within 10 s in 3/3 attempts; manual error button shown after 3 failures.

---

## Journal integrity after crash

```bash
# While a sync transfer is in progress, kill the daemon:
kill -9 $(pgrep adagio-daemon)

# Restart the daemon and let it run one sync cycle
./target/debug/adagio-daemon &
sleep 10

# Check journal integrity via SQLite:
sqlite3 "$HOME/.local/share/adagio/adagio.db" "PRAGMA integrity_check;"
# expect: ok

# Check for orphaned conflict_records (status=conflict but no transfer in progress):
sqlite3 "$HOME/.local/share/adagio/adagio.db" \
  "SELECT count(*) FROM journal_entries WHERE status='error' AND retry_count > 5;"
# expect: 0 (all error entries should have been cleaned up or retried)
```

---

## Quality gates

```bash
cargo clippy -- -D warnings            # 0 warnings
cargo fmt --all --check                # passes
cargo test --lib -p adagio-core        # 133 tests pass
cargo test --lib -p adagio-ipc         # all new IPC tests pass
cargo test --lib -p adagio-daemon      # all daemon unit tests pass
cargo test --lib -p adagio-desktop     # 37 tests pass (all still pass after refactor)
```
