# Feature Specification: Bandwidth Throttling

**Feature Branch**: `009-bandwidth-throttling`

**Created**: 2026-05-30

**Status**: Draft

**Input**: User description: "Configurable upload and download speed limits — a rate limiter between the propagator and the network client, enforcing a byte-rate ceiling per direction per account. Configurable from the desktop Settings panel and from the CLI."

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Limit Upload Speed to Protect the Connection (Priority: P1)

A user on a slow home connection or a shared office network starts Adagio for the first time after configuring a large Nextcloud folder. Without any limit, the initial sync would saturate their upload and make video calls or web browsing unusable. They open Settings, enter 500 Kbps in the upload limit field, and save. From that point on, Adagio respects the cap — sync completes eventually, but the connection remains usable for other applications throughout.

**Why this priority**: Saturating a user's connection is the most disruptive thing Adagio can do. This is the single most requested feature class for any sync client. Without it, users on slow connections cannot run Adagio at all.

**Independent Test**: Configure a 200 Kbps upload limit. Run a sync that uploads at least 10 MB. Measure average throughput over any 10-second window — it must not exceed 204 800 bytes/second. Disable the limit and repeat — the same transfer must complete significantly faster.

**Acceptance Scenarios**:

1. **Given** a user has set an upload limit of 500 Kbps, **When** Adagio uploads files to the server, **Then** the average upload throughput measured over any 10-second window does not exceed 512 000 bytes/second, and the measured rate stays within 10% of the configured ceiling under sustained load.

2. **Given** no upload limit is configured (or the limit is set to 0), **When** Adagio uploads files, **Then** transfers proceed at the full speed available on the connection with no artificial delay.

3. **Given** an upload limit is active, **When** the limit is raised, lowered, or cleared while a transfer is in progress, **Then** the new limit takes effect within 1 second without interrupting or restarting the transfer.

4. **Given** multiple files are uploading concurrently, **When** an upload limit is active, **Then** the combined throughput of all concurrent uploads together stays within the configured ceiling — the limit applies to the total, not per-file.

---

### User Story 2 — Limit Download Speed Independently (Priority: P2)

A user watching a video stream while Adagio downloads a large folder wants to cap the download speed without touching the upload limit. They set a 1 Mbps download limit. Their video plays smoothly because Adagio yields the remaining bandwidth.

**Why this priority**: Upload and download are independent flows on most connections. A user might be fine with full-speed downloads at night but need to throttle downloads during working hours while keeping uploads unlimited (or vice versa).

**Independent Test**: Set a download limit but no upload limit. Run a sync that both uploads and downloads. Measure download throughput (must stay at or below the limit) and upload throughput (must be unconstrained).

**Acceptance Scenarios**:

1. **Given** a download limit is set, **When** Adagio downloads files, **Then** average download throughput over any 10-second window does not exceed the configured limit.

2. **Given** a download limit is set but no upload limit, **When** both an upload and a download are in progress simultaneously, **Then** downloads are throttled to the limit and uploads proceed at full available speed.

3. **Given** both upload and download limits are set to different values, **When** simultaneous transfers occur in both directions, **Then** each direction is throttled independently to its own limit.

---

### User Story 3 — Configure Limits from the Desktop Settings Panel (Priority: P3)

A user who does not use the terminal opens Settings → Sync → Bandwidth, enters numbers in the upload and download limit fields, and saves. The limits take effect immediately and survive an app restart.

**Why this priority**: The desktop app is the primary UI for most users. Configuration must be accessible without requiring CLI knowledge.

**Independent Test**: Open Settings → Sync → Bandwidth. Enter 300 in the upload field. Save. Confirm a sync cycle runs at ≤ 300 Kbps. Close and reopen the app. Confirm the 300 Kbps limit is still shown and still enforced.

**Acceptance Scenarios**:

1. **Given** the user opens Settings → Sync → Bandwidth, **When** they enter a positive number in the upload or download limit field and save, **Then** the limit takes effect within 1 second for all ongoing and future transfers, and the value is persisted to disk.

2. **Given** the user clears both limit fields (or enters 0) and saves, **Then** throttling is disabled for both directions and transfers run at full available speed.

3. **Given** the user enters an invalid value (non-numeric, negative, or an excessively large number), **When** they attempt to save, **Then** the field is highlighted with a validation error and the save is rejected — no partial change is applied.

4. **Given** the app is restarted after limits were saved, **When** the Settings panel is opened, **Then** the previously saved limits are shown and enforced on the first sync cycle.

---

### User Story 4 — Configure Limits from the CLI (Priority: P4)

A headless-server administrator sets bandwidth limits via shell script or cron job without opening any GUI.

**Why this priority**: The CLI is the primary interface for server users; this feature has no value on headless systems without CLI support.

**Independent Test**: Run `adagio bandwidth set --upload-kbps 100`. Confirm `adagio bandwidth status` reports the limit. Trigger a sync and measure upload speed ≤ 100 Kbps. Run `adagio bandwidth clear`. Confirm limits are removed.

**Acceptance Scenarios**:

1. **Given** the daemon is running, **When** the user runs `adagio bandwidth set --upload-kbps 200`, **Then** the upload limit is applied immediately and `adagio bandwidth status` reports "200 Kbps" for upload.

2. **Given** the user runs `adagio bandwidth set` with only one direction flag, **Then** only that direction is changed; the other direction is unchanged.

3. **Given** the user runs `adagio bandwidth clear`, **Then** both upload and download limits are removed and `adagio bandwidth status` reports "Unlimited" for both.

4. **Given** the user runs `adagio bandwidth set --upload-kbps 0`, **Then** this is treated as removing the upload limit (equivalent to `clear` for that direction).

5. **Given** the daemon is not running, **When** `adagio bandwidth set` is run, **Then** the CLI auto-starts the daemon, applies the limit, and exits 0.

---

### User Story 5 — Verify Throttling Is Working (Priority: P5)

A user who set a limit wants to confirm it is actually being respected. They open the Settings bandwidth panel or run `adagio bandwidth status` during an active sync and see both the configured limit and the current measured throughput.

**Why this priority**: Without visibility into live throughput, users cannot tell whether the limit is working or whether the connection is just slow naturally.

**Independent Test**: During an active large upload, open Settings → Sync → Bandwidth. The "Current upload speed" figure must be updating in near-real-time (within 2 seconds of change) and must be at or below the configured limit.

**Acceptance Scenarios**:

1. **Given** Adagio is actively transferring files, **When** the user checks the bandwidth view (Settings panel or CLI), **Then** both the configured limit (or "Unlimited") and the current measured speed are shown for upload and download separately.

2. **Given** the user checks bandwidth status when no transfer is in progress, **Then** the current speed shows 0 (or "idle") and the configured limits are still shown.

3. **Given** the throughput display is open, **When** a transfer speed changes by more than 20%, **Then** the displayed current speed updates within 2 seconds.

---

### Edge Cases

- **What if the configured limit is lower than the minimum granularity of the rate limiter?** The system enforces a minimum practical limit of 10 Kbps. If a user sets a value below 10 Kbps, it is silently clamped to 10 Kbps and a notice is shown.
- **What if the network is slower than the configured limit?** The limit is a ceiling, not a guarantee. If the network is slower than the cap, transfers proceed at the network's actual speed with no interference.
- **What if the app crashes while a transfer is in progress under a limit?** Limits are stored durably in config. On restart, the limit is re-applied before the next sync cycle begins.
- **What if two accounts have different limits?** Each account has its own independent limits. The per-account limit applies to all transfers for that account's pairs.
- **What if the user sets a limit below what is currently being transferred?** The transfer is throttled to the new lower limit within 1 second; the transfer is not restarted.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to configure a maximum upload speed in kilobits per second for each connected account, independently of the download limit.

- **FR-002**: Users MUST be able to configure a maximum download speed in kilobits per second for each connected account, independently of the upload limit.

- **FR-003**: Setting either limit to 0 MUST disable throttling for that direction (unlimited).

- **FR-004**: When a limit is active, the combined throughput of all concurrent transfers in that direction MUST NOT exceed the configured limit, measured as an average over any consecutive 10-second window.

- **FR-005**: The configured limits MUST take effect within 1 second of being saved — including while transfers are already in progress. In-progress transfers MUST NOT be interrupted or restarted.

- **FR-006**: Configured limits MUST persist across app restarts and daemon restarts. They MUST be stored alongside the account's other configuration.

- **FR-007**: The desktop Settings panel MUST include a Bandwidth section showing upload limit, download limit, current upload speed, and current download speed. Users MUST be able to set and clear limits from this panel.

- **FR-008**: The CLI MUST support `adagio bandwidth status`, `adagio bandwidth set`, and `adagio bandwidth clear` commands per the contract described in the feature description.

- **FR-009**: `adagio bandwidth status` MUST show both the configured limits and the current measured throughput for each direction.

- **FR-010**: Invalid inputs (non-numeric, negative, or values below the enforced minimum of 10 Kbps) MUST be rejected with a clear error message. No partial configuration change MUST be applied on validation failure.

- **FR-011**: If multiple accounts are configured, each account has its own independent bandwidth limits. The CLI and Settings panel MUST allow specifying which account's limits to modify (defaulting to the only account if there is just one).

- **FR-012**: Limits apply to file content transfer bytes only. Control traffic (metadata queries, file listings, authentication) is not subject to bandwidth limits.

### Key Entities

- **BandwidthConfig**: The stored configuration for one account's limits. Attributes: account ID, upload limit in Kbps (0 = unlimited), download limit in Kbps (0 = unlimited).

- **BandwidthStatus**: The live view combining configuration and measurement. Attributes: configured upload limit, configured download limit, current upload speed (bytes/sec), current download speed (bytes/sec), is throttling active (boolean).

- **TransferDirection**: An enum distinguishing upload from download, used to apply limits independently.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Under a 500 Kbps upload limit and sustained upload load, the average throughput measured over any 10-second window is between 450 Kbps and 512 Kbps (within 10% of the configured limit in either direction). Verified by automated transfer benchmark.

- **SC-002**: When no limit is configured, measured throughput during a benchmark transfer is at least 80% of the available connection speed — confirming no artificial overhead is introduced by the throttling infrastructure even when inactive.

- **SC-003**: A limit change (including disabling) takes effect in under 1 second when applied while a transfer is in progress, without the transfer restarting. Verified by timing the change in measured throughput after a settings update.

- **SC-004**: Limits configured in the desktop app are still applied after the daemon is killed and restarted — 100% of saved limits survive a restart. Verified by configuring a limit, restarting the daemon, and measuring throughput on the next cycle.

- **SC-005**: The live throughput display in the Settings panel or CLI updates within 2 seconds of a meaningful speed change during an active transfer. Verified by manual observation during a benchmark transfer.

- **SC-006**: A user with no CLI experience can configure, verify, and clear a bandwidth limit using only the desktop Settings panel in under 2 minutes on their first attempt.

---

## Assumptions

- **Per-account granularity is sufficient for v1**: Users with multiple accounts can set different limits per account, but all pairs under the same account share one limit. Per-pair limits are explicitly out of scope.

- **Kbps is the unit in the UI**: Kilobits per second (not kilobytes) aligns with how ISPs advertise connection speeds, making it easier for users to set limits relative to their plan.

- **Live throughput is approximate**: The current speed display is a rolling average over the last few seconds. It is informational only — the throttler enforces the hard limit, not the display.

- **Minimum enforced limit of 10 Kbps**: Values below 10 Kbps are impractical and would stall sync indefinitely. The system clamps to 10 Kbps as a safety floor.

- **Limits do not interact with pause/resume**: Setting a limit does not replace pausing. A paused account transfers nothing regardless of its bandwidth limit.

- **The Settings panel defaults to the first account if more than one is configured**: Multi-account bandwidth UI presents an account selector dropdown when multiple accounts exist.

- **Control traffic exclusion**: WebDAV PROPFIND, capabilities checks, and authentication requests are not counted against the bandwidth limit, as they are small and infrequent.
