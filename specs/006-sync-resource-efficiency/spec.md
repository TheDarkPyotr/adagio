# Feature Specification: Sync Resource Efficiency

**Feature Branch**: `006-sync-resource-efficiency`

**Created**: 2026-05-29

**Status**: Draft

**Input**: User description: "Story 1: Reliable Sync That Stays Out of the Way (P1) — As a daily Nextcloud user I want sync to complete in a bounded amount of time and then stay quiet so that my fans aren't spinning and my files aren't locked while I work."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Reliable Sync That Stays Out of the Way (Priority: P1)

A daily Nextcloud user runs Adagio continuously in the background. After a sync
cycle completes with no changes, the client goes truly idle: CPU drops to
near-zero, no files are held open, and logs show no rapid-fire retries.
The client never interferes with applications that have files open, and over a
full working day it does not silently consume more and more memory.

**Why this priority**: This is the "first do no harm" story. A sync client that
spins the fan, locks files, or leaks memory will be uninstalled within a day.
All other features are irrelevant if the process is a resource burden.

**Independent Test**: Run the client against a fully-synced pair for 24 hours
with no local or remote changes; measure CPU (averaged over 5-min windows),
peak vs. baseline RSS, and scan the log for retry bursts. All four acceptance
criteria must pass independently.

**Acceptance Scenarios**:

1. **Given** a fully-synced folder with no local or remote changes,
   **When** the client has been idle for 60 seconds,
   **Then** background CPU usage averaged over the next 5 minutes is under 1%
   on a reference machine (4-core, 16 GB RAM).

2. **Given** a folder containing files open by other applications (editors,
   Obsidian, IDEs),
   **When** the client polls or syncs those files,
   **Then** those applications never report the file as locked, busy, or
   inaccessible.

3. **Given** the client has been running continuously for 24 hours,
   **When** resident memory is measured,
   **Then** it has not grown by more than 20% compared to its first-minute
   baseline.

4. **Given** any sync operation,
   **When** it completes,
   **Then** it does so without entering a retry loop visible in logs as more
   than 3 identical requests per second.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: After a sync cycle completes with zero changes detected, the
  engine MUST NOT schedule work more frequently than the configured
  `scan_interval_secs`.
- **FR-002**: Local file scanning MUST use stat-only access (no exclusive open,
  no write lock) on all three platforms (Linux, macOS, Windows).
- **FR-003**: No file descriptor acquired during a scan cycle MAY be held open
  past the end of that cycle.
- **FR-004**: The retry backoff MUST enforce a per-path rate cap of at most
  3 network requests per second for any single remote path.
- **FR-005**: RSS measured at `t = 24h` MUST NOT exceed RSS at `t = 1min` by
  more than 20%.
- **FR-006**: All scan and retry metrics (cycle duration, retry count per path,
  RSS sample) MUST be emitted as structured log events at DEBUG level.

### Key Entities

- **IdleGuard**: Ensures the cycle scheduler sleeps for at least `scan_interval_secs`
  after a no-change cycle before issuing the next PROPFIND.
- **StatScanner**: The local file scanner limited to stat(2)/metadata reads; no
  exclusive file opens.
- **RateLimiter**: Per-path token-bucket that caps outgoing retries to ≤3 req/s.
- **MemorySampler**: Samples RSS at startup and at each cycle completion; emits
  a WARN log when growth exceeds 15% (early warning before the 20% hard limit).

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: CPU ≤ 1% (5-min average) after 60 s of idle — automated benchmark.
- **SC-002**: Zero file-lock events reported by inotify / FSEvents / ReadDirectoryChangesW
  during a scan of a folder containing open files.
- **SC-003**: RSS at 24 h ≤ 1.20 × RSS at 1 min — automated long-run regression test.
- **SC-004**: Log grep for `"retries_per_second"` field shows value ≤ 3.0 for all
  entries over a 30-minute soak run.

---

## Assumptions

- The reference machine for CPU benchmarks is a 4-core, 16 GB RAM laptop running
  Linux with a standard desktop workload in the background.
- "Idle" is defined as: zero pending uploads/downloads, no conflicts awaiting
  resolution, and no remote changes detected in the most recent scan.
- File-lock checking is defined per-platform: Linux uses `lsof`/`/proc/fd`; macOS
  uses `lsof`; Windows uses `SysInternals handle64` or equivalent.
- Memory baseline is the RSS reading taken 60 seconds after startup, after the
  first sync cycle has completed.
