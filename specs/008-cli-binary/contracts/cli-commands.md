# CLI Command Contract: adagio

**Version**: 1.0 | **Binary**: `adagio`

This document defines the complete command syntax, output contract, and exit code
contract for the `adagio` CLI binary.

---

## Global flags

| Flag | Short | Description |
|------|-------|-------------|
| `--json` | — | Output JSON to stdout instead of human-readable text |
| `--help` | `-h` | Print help for the current command/subcommand |
| `--version` | `-V` | Print binary version |

`--json` is **global**: it applies to every subcommand regardless of position.

---

## Exit codes

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Usage error (missing required argument, unknown flag, etc.) — handled by clap |
| `2` | Daemon returned an application-level error |
| `3` | Daemon not reachable and could not be started |

---

## Output contract

**Human mode** (default):
- Column-aligned text to stdout
- Errors to stderr
- Exit code reflects success/failure

**JSON mode** (`--json`):
- stdout: valid JSON only — no ANSI codes, headers, borders
- stderr: error messages
- Exit code reflects success/failure

---

## Command catalogue

### `adagio status`

Show current sync state for all configured pairs.

```
adagio status [--json]
```

**Human output** (table):
```
PAIR ID       LOCAL                REMOTE   STATUS   LAST SYNCED
8dc02447      /home/luca/adagio-test  /     idle     2026-05-30 06:28
990c9e9e      /home/luca/adagio-test  /     idle     2026-05-30 06:28
```

**JSON output**: array of pair status objects from the daemon.

---

### `adagio sync [PAIR_ID]`

Trigger an immediate sync cycle.

```
adagio sync [--json] [PAIR_ID]
```

- Without `PAIR_ID`: syncs all pairs.
- With `PAIR_ID`: syncs only that pair.
- Auto-starts daemon if not running.

**Output**: "Sync triggered" (human) or `{"ok": true}` (JSON).

---

### `adagio pause` / `adagio resume`

Pause or resume all sync activity.

```
adagio pause  [--json]
adagio resume [--json]
```

**Output**: "Sync paused" / "Sync resumed" (human) or `{"ok": true}` (JSON).

---

### `adagio activity`

Show recent activity log.

```
adagio activity [--json] [--limit N] [--filter edit|sync|conflict]
```

**Options**:
- `--limit N` (default: 50)
- `--filter` — one of `edit`, `sync`, `conflict`

**Human output** (table):
```
TIME                  ACTION      FILE
2026-05-30 06:28:15   downloaded  upload-2026-05/report.pdf
2026-05-30 06:28:10   uploaded    notes.txt
```

**JSON output**: array of activity entry objects from the daemon.

---

### `adagio pairs list`

```
adagio pairs list [--json]
```

**Human output**:
```
PAIR ID    LOCAL                    REMOTE   ACCOUNT
8dc02447   /home/luca/adagio-test   /        alice@cloud.example.com
```

**JSON output**: array of pair objects from the daemon.

---

### `adagio pairs add`

```
adagio pairs add --local PATH --remote PATH --account ACCOUNT_ID [--json]
```

**Output**: The created pair object (human: key-value summary, JSON: pair object).

---

### `adagio pairs remove`

```
adagio pairs remove PAIR_ID [--delete-local-files] [--json]
```

**Output**: "Pair removed" (human) or `{"ok": true}` (JSON).

---

### `adagio accounts list`

```
adagio accounts list [--json]
```

**Human output**:
```
ACCOUNT ID   DISPLAY NAME   SERVER                     USERNAME
acc-abc123   Alice          https://cloud.example.com  alice
```

**JSON output**: array of account objects from the daemon.

---

### `adagio accounts remove`

```
adagio accounts remove ACCOUNT_ID [--json]
```

**Output**: "Account removed" (human) or `{"ok": true}` (JSON).

---

### `adagio conflicts list`

```
adagio conflicts list [PAIR_ID] [--json]
```

**Human output**:
```
CONFLICT ID   FILE                LOCAL SIZE   SERVER SIZE   DETECTED
abc-1234      docs/report.pdf     204 KB        198 KB        2026-05-29
```

**JSON output**: array of conflict objects from the daemon.

---

### `adagio conflicts resolve`

```
adagio conflicts resolve CONFLICT_ID --keep local|remote|both [--json]
```

**Output**: "Conflict resolved" (human) or `{"ok": true}` (JSON).

---

### `adagio conflicts dismiss`

```
adagio conflicts dismiss [--json]
```

**Output**: "N conflicts dismissed" (human) or `{"dismissed_count": N}` (JSON).

---

### `adagio daemon start`

```
adagio daemon start [--json]
```

Starts the daemon if not running. Exits within 5 seconds.

**Output**: "Daemon started" / "Daemon already running" (human) or
`{"started": true/false}` (JSON).

---

### `adagio daemon stop`

```
adagio daemon stop [--json]
```

Requests graceful shutdown. Waits up to 30 seconds.

**Output**: "Daemon stopped" (human) or `{"stopped": true}` (JSON).
Exit code 0 if stopped successfully or was already stopped.

---

### `adagio daemon status`

```
adagio daemon status [--json]
```

**Human output**:
```
Status:   running
Uptime:   2h 14m
Version:  0.1.0
Pairs:    2
```

**JSON output**: daemon status object from the daemon.

---

## Error output shapes

When a command fails:

**Human mode**: error message on stderr, non-zero exit.

**JSON mode**: `{"error": "description of error"}` on stdout, error details on
stderr, non-zero exit.
