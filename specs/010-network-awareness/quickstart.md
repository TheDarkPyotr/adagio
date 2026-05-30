# Quickstart Validation: Network Awareness (010)

End-to-end validation for all five user stories.

---

## Prerequisites

```bash
cargo build -p adagio-daemon -p adagio-cli
./target/debug/adagio-daemon --config-dir ~/.config/ai.neuralagent.adagio/ &
sleep 1
```

---

## US1 + US2 — Metered and battery policies (P1+P2)

```bash
# Check current status (all allow by default)
./target/debug/adagio network status
# Expected: Metered=No/Unknown, Battery=No/Yes, effective_action=allow

# Set metered=pause
./target/debug/adagio network set --on-metered pause
./target/debug/adagio network status --json | python3 -m json.tool
# Expected: policy.on_metered="pause"

# Set battery=throttle at 200 Kbps
./target/debug/adagio network set --on-battery throttle --throttle-kbps 200
./target/debug/adagio network status
# If on battery: effective_action=throttle (200 Kbps)
# If on AC:      effective_action=allow

# Reset all
./target/debug/adagio network set --on-metered allow --on-battery allow --throttle-kbps 0
./target/debug/adagio network status
# Expected: effective_action=allow, reason=""
```

**Pass criteria**: Status reflects configured policy; throttle/pause activates when conditions match.

---

## US3 — SSID block list (P3)

```bash
# Block a fake SSID
./target/debug/adagio network block-ssid "TestNetwork-Blocked"
./target/debug/adagio network list-blocked
# Expected: ["TestNetwork-Blocked"]

# If you can switch to that SSID, network status should show paused
# Otherwise verify the list is persisted:
pkill adagio-daemon
./target/debug/adagio-daemon --config-dir ~/.config/ai.neuralagent.adagio/ &
sleep 1
./target/debug/adagio network list-blocked
# Expected: ["TestNetwork-Blocked"] (survived restart)

# Unblock
./target/debug/adagio network unblock-ssid "TestNetwork-Blocked"
./target/debug/adagio network list-blocked
# Expected: []
```

**Pass criteria**: SSID list persists across daemon restarts; add/remove work correctly.

---

## US4 — CLI status display (P4)

```bash
./target/debug/adagio network status
# Table shows: metered, battery, SSID, effective action + reason

./target/debug/adagio network status --json
# Full JSON object with metered, on_battery, ssid, effective_action, reason, policy
```

**Pass criteria**: All fields present; `effective_action` matches current detected state and policy.

---

## US5 — Settings panel (P5)

```bash
cd crates/adagio-desktop
npm run tauri:dev
```

1. Open **Settings → Sync → Network**
2. Verify: dropdowns for "On metered" and "On battery", Kbps input, SSID block list
3. Set "On metered" to "Pause", click Save
4. Run `./target/debug/adagio network status --json` — verify `policy.on_metered = "pause"`
5. Add SSID "TestWifi" via the + button in the UI
6. Verify it appears in `adagio network list-blocked`
7. Close and reopen the app — verify settings are persisted

**Pass criteria**: UI changes are reflected in CLI; CLI changes are reflected in UI (3s refresh).

---

## Quality gates

```bash
cargo clippy -- -D warnings              # 0 errors
cargo fmt --all --check                  # passes
cargo test --lib -p adagio-core          # 139+ pass (including new network tests)
cargo test -p adagio-daemon              # all pass
cargo test -p adagio-cli                 # all pass
npm run test                             # 99+ pass
```

---

## Graceful degradation tests

```bash
# Simulate D-Bus unavailable (stop NetworkManager — ONLY on a dev VM)
# sudo systemctl stop NetworkManager
./target/debug/adagio network status
# Expected: metered=false, ssid=null, no crash, effective_action=allow

# Restore
# sudo systemctl start NetworkManager
```

**Pass criteria**: Daemon continues running; no panic; conditions treated as allow.
