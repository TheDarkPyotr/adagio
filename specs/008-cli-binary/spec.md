# Feature Specification: CLI Binary

**Feature Branch**: `008-cli-binary`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "CLI Binary — a standalone command-line tool (`adagio`) that connects to the running background daemon via the local IPC socket and lets users inspect and control sync from the terminal, shell scripts, and cron jobs."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Check Sync Status from the Terminal (Priority: P1)

A developer or power user wants to know whether their sync is running, paused, or has
errors right now — without opening a window. They run a single command, see a
concise summary of every configured sync pair, and move on. The same command works
unattended in a monitoring script by adding `--json`.

**Why this priority**: The most common CLI use case is passive inspection. Every other
story (triggering sync, resolving conflicts, scripting) depends on this foundation.
It is the minimal viable CLI.

**Independent Test**: With a running daemon and at least one sync pair, `adagio status`
prints a table of pairs and statuses. `adagio status --json` prints valid JSON that
a script can consume. Both complete in under 1 second.

**Acceptance Scenarios**:

1. **Given** the daemon is running and one or more sync pairs are configured, **When**
   the user runs `adagio status`, **Then** each pair is listed with its local folder,
   remote path, current status (idle / syncing / paused / error), and the time it last
   completed a sync cycle.

2. **Given** the daemon is running, **When** the user runs `adagio status --json`,
   **Then** stdout contains a valid JSON array (one object per pair) and nothing else;
   no colour codes, no headers, no decorations.

3. **Given** the daemon is not running, **When** the user runs `adagio status`,
   **Then** the CLI prints a clear error message to stderr and exits with code 3
   (daemon not reachable).

4. **Given** no sync pairs are configured, **When** the user runs `adagio status`,
   **Then** the output says "No sync pairs configured" (human-readable) or an empty
   array (JSON mode), and the exit code is 0.

---

### User Story 2 — Trigger Sync from a Script or Cron Job (Priority: P2)

A headless-server user or CI pipeline needs to kick off a sync cycle without any GUI.
They write a cron entry or a shell script that calls `adagio sync`. The command should
auto-start the daemon if it is not already running so the script works reliably even
after a reboot.

**Why this priority**: Headless and automated operation is the primary use case that
a CLI unlocks over the desktop app. Without it, a server user gains little from having
a CLI at all.

**Independent Test**: Kill the daemon, run `adagio sync`, verify the daemon starts,
a sync cycle completes for all pairs, and the command exits 0. Then run `adagio sync
PAIR_ID` and verify only that pair cycles.

**Acceptance Scenarios**:

1. **Given** the daemon is running, **When** the user runs `adagio sync`, **Then** an
   immediate sync cycle is triggered for every configured pair and the command exits
   with code 0.

2. **Given** the daemon is NOT running, **When** the user runs `adagio sync`, **Then**
   the CLI starts the daemon automatically, waits up to 5 seconds for it to be ready,
   triggers the sync, and exits 0.

3. **Given** the user provides a specific pair identifier, **When** they run
   `adagio sync PAIR_ID`, **Then** only that pair syncs; other pairs are unaffected.

4. **Given** the daemon cannot be started (e.g., binary not found, permission error),
   **When** the user runs `adagio sync`, **Then** the CLI prints the error to stderr
   and exits with code 3.

---

### User Story 3 — Manage Conflicts from the Terminal (Priority: P3)

A headless user or remote administrator cannot open the GUI to see or resolve sync
conflicts. They need to list pending conflicts and choose a resolution for each without
a visual interface.

**Why this priority**: Conflicts block further syncing for the affected files. On a
headless server there is no other way to resolve them.

**Independent Test**: Create a conflict (modify the same file on both local and
server). Run `adagio conflicts list`, note the conflict ID, then run
`adagio conflicts resolve ID --keep local`. Verify the conflict is resolved and the
file on the server matches the local version.

**Acceptance Scenarios**:

1. **Given** at least one conflict exists, **When** the user runs `adagio conflicts
   list`, **Then** each conflict is shown with its file path, local size and
   modification time, server size and modification time, and the date it was detected.

2. **Given** a known conflict ID, **When** the user runs `adagio conflicts resolve
   ID --keep local`, **Then** the local version is kept, the server copy is
   overwritten, the conflict record is cleared, and the command exits 0.

3. **Given** `--keep remote` is specified, **When** the command runs, **Then** the
   server version replaces the local file and the conflict is cleared.

4. **Given** `--keep both` is specified, **When** the command runs, **Then** the local
   file is preserved and the server version is saved alongside it with a clearly
   labelled name; both files exist after the command exits.

5. **Given** no conflicts exist, **When** the user runs `adagio conflicts list`,
   **Then** the output says "No pending conflicts" (human) or an empty array (JSON)
   and exits 0.

6. **Given** `adagio conflicts dismiss`, **When** the command runs, **Then** all
   pending conflicts are dismissed without any file I/O and the command exits 0.

---

### User Story 4 — Use CLI Output in Shell Scripts (Priority: P4)

A developer or DevOps engineer wants to pipe `adagio` output to `jq`, `grep`, or
other tools. They need every command to support a `--json` flag that suppresses all
decorative output and emits only machine-parseable data on stdout.

**Why this priority**: Without reliable machine-readable output, the CLI cannot be
used in scripts — which is half its value proposition. This story is a horizontal
contract that applies to all other commands.

**Independent Test**: For every command that produces output, run it with `--json`
and pipe to `jq .` — no parse error may occur. Verify exit codes propagate correctly
and that nothing goes to stdout except JSON.

**Acceptance Scenarios**:

1. **Given** any command that produces output (status, pairs list, conflicts list,
   activity, accounts list), **When** `--json` is appended, **Then** stdout contains
   only valid JSON; no ANSI colour codes, spinners, headers, borders, or trailing
   newlines appear on stdout.

2. **Given** a command that produces list output with `--json`, **When** the result
   is piped to a JSON processor, **Then** the processor receives a valid JSON array
   where each element represents one item.

3. **Given** a command fails (daemon error, invalid argument), **When** `--json` is
   in use, **Then** stderr receives the error message, stdout receives a JSON error
   object `{"error": "..."}`, and the exit code is non-zero.

4. **Given** the user runs `adagio status --json` in a cron script and the daemon is
   not running, **Then** the exit code is 3 (not 0), allowing the script to detect
   the failure reliably.

---

### User Story 5 — Control the Daemon Lifecycle from the Terminal (Priority: P5)

A system administrator or advanced user needs to start, stop, and check the daemon
without opening any GUI — for example, to restart it after a config change, to stop
it before a maintenance window, or to verify it is running in a monitoring script.

**Why this priority**: Completing the "no GUI required" story. Once a user can manage
all aspects of the daemon from the terminal, the desktop app becomes optional for
power users.

**Independent Test**: Kill the daemon. Run `adagio daemon start`. Verify daemon
is running within 5 seconds. Run `adagio daemon status` and confirm it reports
running + uptime. Run `adagio daemon stop`. Verify the daemon process is gone.

**Acceptance Scenarios**:

1. **Given** the daemon is not running, **When** the user runs `adagio daemon start`,
   **Then** the daemon starts in the background and the command exits 0 within 5
   seconds; no window or GUI appears.

2. **Given** the daemon is already running, **When** the user runs `adagio daemon
   start`, **Then** the command exits 0 immediately with a message "Daemon is already
   running" (human) or `{"running": true}` (JSON).

3. **Given** the daemon is running, **When** the user runs `adagio daemon status`,
   **Then** the output includes: running status (yes/no), uptime, version, and number
   of configured pairs.

4. **Given** the daemon is running, **When** the user runs `adagio daemon stop`,
   **Then** the daemon finishes any in-flight sync transfers (up to 30 seconds), stops
   cleanly, and the command exits 0.

5. **Given** the daemon is not running, **When** the user runs `adagio daemon stop`,
   **Then** the command exits 0 with a message "Daemon is not running".

---

### Edge Cases

- **What if the daemon socket exists but no daemon is responding?** The CLI detects
  a stale socket (ping times out), removes it, starts a fresh daemon, and retries.
- **What if a pair ID is provided but does not exist?** The CLI prints an error to
  stderr and exits with code 2.
- **What if a conflict ID is provided but the conflict is already resolved?** The CLI
  prints "Conflict already resolved" and exits 0.
- **What if the daemon returns a partial error (some pairs succeed, some fail)?** The
  CLI reports each outcome individually and exits with code 2.
- **What if the user runs `adagio` with no subcommand?** The CLI prints usage help
  and exits with code 1.
- **What if `--json` and `--help` are combined?** `--help` takes precedence; help
  text is printed in human-readable format regardless of `--json`.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A binary named `adagio` MUST be available as a standalone executable
  that users can invoke from any terminal session on the same machine.

- **FR-002**: The binary MUST connect to the running background daemon via the
  local IPC socket. It MUST NOT start its own sync engine or hold any persistent
  state.

- **FR-003**: If the daemon is not running when a command requires it, the binary
  MUST attempt to start the daemon automatically before executing the command.

- **FR-004**: Every command MUST complete and exit; the binary MUST NOT run
  persistently or wait for events unless explicitly documented (e.g., `daemon stop`
  waits for drain).

- **FR-005**: All commands that produce structured output MUST support a `--json`
  flag. When `--json` is active, stdout MUST contain only valid JSON and nothing
  else.

- **FR-006**: The binary MUST use specific exit codes: 0 (success), 1 (usage error),
  2 (daemon returned an error), 3 (daemon not reachable).

- **FR-007**: The `status` command MUST display the current sync state for each
  configured pair, including status label and last-synced timestamp.

- **FR-008**: The `sync` command MUST trigger an immediate sync cycle. When a pair
  ID is provided, only that pair syncs; otherwise all pairs sync.

- **FR-009**: The `pause` and `resume` commands MUST halt and restart sync activity
  across all pairs respectively.

- **FR-010**: The `pairs list` command MUST display each configured pair with its
  local folder path, remote folder path, and account identifier.

- **FR-011**: The `pairs add` command MUST create a new sync pair given a local path,
  a remote path, and an account identifier, and persist it to the daemon's
  configuration.

- **FR-012**: The `pairs remove` command MUST delete a sync pair. With
  `--delete-local-files` it MUST also remove all synced files from the local folder.
  Without that flag, local files MUST be preserved.

- **FR-013**: The `accounts list` command MUST display each connected account with
  its display name, server URL, and username.

- **FR-014**: The `accounts remove` command MUST remove an account and all of its
  associated sync pairs from the daemon's configuration.

- **FR-015**: The `conflicts list` command MUST display all pending (unresolved)
  conflicts. When a pair ID is provided, only that pair's conflicts are shown.

- **FR-016**: The `conflicts resolve` command MUST resolve a conflict given its ID
  and a resolution choice (`local`, `remote`, or `both`). The correct file I/O MUST
  be performed and the conflict record cleared.

- **FR-017**: The `conflicts dismiss` command MUST clear all pending conflicts
  without performing any file I/O, marking them as resolved.

- **FR-018**: The `activity` command MUST display recent sync activity entries,
  optionally limited in count and filtered by kind.

- **FR-019**: The `daemon start` command MUST start the background daemon process
  in the background and exit within 5 seconds of the daemon becoming ready.

- **FR-020**: The `daemon stop` command MUST request a graceful daemon shutdown.
  It MUST wait up to 30 seconds for in-flight transfers to complete before reporting
  success.

- **FR-021**: The `daemon status` command MUST report whether the daemon is
  currently running, its uptime, and its version.

- **FR-022**: When the daemon is not running and cannot be started, the binary
  MUST exit with code 3 and print a human-readable error on stderr explaining
  what went wrong.

### Key Entities

- **Command**: A user-typed invocation of the binary with a subcommand, optional
  arguments, and optional `--json` flag. Attributes: subcommand name, positional
  arguments, named flags, output mode.

- **Pair Summary**: The information displayed for one sync pair by `status` and
  `pairs list`. Attributes: pair ID, local path, remote path, account display
  name, current status, last-synced timestamp.

- **Conflict Entry**: The information displayed for one conflict by `conflicts
  list`. Attributes: conflict ID, file path, local size, local modification time,
  server size, server modification time, detected timestamp.

- **Activity Entry**: One record from the sync activity log. Attributes: action
  verb (uploaded/downloaded/deleted/conflict), file path, timestamp, pair
  identifier.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Every command that returns data completes and exits in under 2 seconds
  when the daemon is already running, measured on a reference machine with a normal
  network connection to the Nextcloud server.

- **SC-002**: `adagio sync` auto-starts the daemon and triggers the first sync cycle
  within 8 seconds on a machine where the daemon binary is available but not yet
  running.

- **SC-003**: 100% of commands that produce structured output return valid JSON when
  `--json` is passed — verified by piping to a JSON validator with zero parse errors.

- **SC-004**: Exit codes are correct in 100% of tested scenarios: 0 for success, 1
  for usage errors, 2 for daemon errors, 3 for unreachable daemon.

- **SC-005**: A cron-job user with no GUI access can perform the complete conflict
  resolution workflow — list, inspect, and resolve — using only `adagio` commands
  in under 5 minutes for a typical one-conflict case.

- **SC-006**: The binary works correctly on all three target platforms (Linux,
  macOS, Windows) with the same command syntax and exit-code contract.

---

## Assumptions

- **Daemon binary co-location**: When auto-starting the daemon, the CLI locates
  `adagio-daemon` as a sibling of its own executable (same directory). On packaged
  distributions both binaries are installed together.

- **IPC socket path is shared**: The CLI uses the same platform-specific socket path
  as the desktop app and daemon (`$XDG_RUNTIME_DIR/adagio/daemon.sock` on Linux,
  `~/Library/Application Support/adagio/daemon.sock` on macOS). No configuration
  file is needed to find the socket.

- **Account creation requires the GUI**: The `accounts add` subcommand is out of
  scope. Adding an account requires an OAuth2 browser flow that the CLI cannot
  perform. Existing accounts (added via the desktop app) are fully manageable
  from the CLI.

- **Conflict resolution file I/O happens in the daemon**: When `conflicts resolve`
  is called, the actual file download/upload is performed by the daemon, not the
  CLI. The CLI sends a resolve command and reports the result.

- **Single user, single daemon**: The CLI targets a per-user daemon. Multi-user
  or system-daemon scenarios are out of scope.

- **Config dir discovery**: The CLI derives the correct config dir using the same
  platform logic as the daemon (XDG on Linux, Application Support on macOS,
  AppData on Windows) so it can pass `--config-dir` to the daemon if starting it.

- **No paging or interactive prompts**: Output is printed all at once. For very
  long lists (hundreds of files) the user is expected to use shell tools like
  `less` or `jq`. The CLI never blocks waiting for keyboard input except via
  explicit `--interactive` flags (which are out of scope for this feature).
