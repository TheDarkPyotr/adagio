# Feature Specification: Bulk Upload Driver

**Feature ID**: 011  
**Created**: 2026-05-30  
**Status**: Draft  
**Priority**: P1 (parallel uploads) → P5 (automatic activation)

---

## Overview

The first time a user syncs a large local folder to an empty Nextcloud remote, Adagio's standard sync cycle is too slow: it uploads files one at a time, restarts from zero after any interruption, and adds overhead by re-scanning both sides on every iteration. The Bulk Upload Driver is a dedicated fast path for this initial-sync scenario. It uploads files in parallel, resumes at the file level after interruptions, and uses chunked multipart for large files — delivering the initial sync in a fraction of the time of the standard path.

---

## Problem Statement

Users syncing large collections for the first time (photography archives, document libraries, video projects) face hours or days of upload time because the standard sync engine was designed for steady-state maintenance, not bulk ingestion. The lack of resume means any interruption — network hiccup, sleep, crash — forces a full restart. The single-file-at-a-time model wastes available bandwidth and connection concurrency.

---

## User Stories

### US1 — Parallel uploads complete faster than sequential (P1)
As a user syncing a large folder for the first time, I want uploads to run in parallel so that the initial sync finishes in a fraction of the time it would take sequentially.

**Acceptance Criteria:**

- GIVEN a local folder with 100 files  
  WHEN the bulk upload driver runs with 8 workers  
  THEN wall-clock time is less than 8× the time for a single sequential upload of equal total size

- GIVEN the bulk upload completes  
  THEN all files appear on the remote  
  AND all journal entries have status Synced

### US2 — Resume after interruption (P2)
As a user whose sync was interrupted mid-upload, I want the bulk upload to resume from where it stopped rather than re-uploading already-completed files.

**Acceptance Criteria:**

- GIVEN the bulk upload was interrupted after N files were uploaded  
  WHEN the daemon restarts and triggers another sync  
  THEN only the remaining files are uploaded  
  AND the already-uploaded files are not re-uploaded

### US3 — Chunked upload for large files (P3)
As a user with large files such as videos or disk images, I want files above a size threshold to be uploaded in chunks so that an interrupted large-file upload can resume from the last committed chunk.

**Acceptance Criteria:**

- GIVEN a file larger than the configured chunk threshold (default 10 MB)  
  WHEN the bulk upload driver processes it  
  THEN the file is uploaded using the chunked multipart protocol

- GIVEN a chunked upload is interrupted at chunk N  
  WHEN the upload is retried  
  THEN only chunks N+1 onwards are sent; chunks 0 through N are not re-transmitted

### US4 — Progress visible in UI and CLI (P4)
As a user waiting for a large initial sync to complete, I want to see real-time progress so I can estimate when it will finish.

**Acceptance Criteria:**

- GIVEN the bulk upload driver is running  
  WHEN I open the desktop app or run the sync status command  
  THEN I see: files uploaded / total files, bytes transferred / total bytes, estimated time remaining

- The progress display updates at least once per second during active transfers

### US5 — Automatic activation (P5)
As a user, I want the bulk upload to activate automatically when conditions are right, without manual configuration.

**Acceptance Criteria:**

- GIVEN a sync pair has just been created and the remote folder is empty  
  AND the local folder has more files than the configured threshold (default 50)  
  WHEN the first sync cycle runs  
  THEN the bulk upload driver is used automatically

- GIVEN the bulk upload completes  
  THEN the standard sync cycle takes over transparently for ongoing maintenance

---

## Functional Requirements

### FR-1: Parallel worker pool
- The driver uploads multiple files concurrently using a configurable worker pool
- Default workers: 8; configurable per sync pair in the range 1–32
- Each worker independently handles one file at a time
- Worker count is capped at the number of remaining files to avoid idle workers

### FR-2: File-level resume
- Before starting, the driver queries the journal for all already-Synced entries in the pair
- Already-Synced files are excluded from the upload queue
- Files with permanent errors (e.g. path-too-long) are excluded from the queue
- On restart after interruption, the driver rebuilds the queue from current journal state — only unfinished files are processed

### FR-3: Chunked multipart for large files
- Files at or above the chunk threshold (default 10 MB) use the chunked multipart upload protocol
- The driver checks for existing partial uploads on the server before starting a chunked upload; if chunks already exist, it resumes from the last committed chunk
- Files below the threshold use the standard single-PUT upload path

### FR-4: Journal writes on completion
- Each successfully uploaded file is immediately recorded in the journal with status Synced, its server-assigned identifier, and checksum
- Journal writes are atomic — a file is either fully recorded or not recorded at all
- A file whose upload fails is recorded with status Error and the failure reason

### FR-5: Progress events
- The driver emits progress events at least once per second during active transfers
- Each event includes: files completed, files total, bytes transferred, bytes total, and estimated time remaining
- Progress events are compatible with the existing sync status IPC contract so the desktop app and CLI display them without additional changes

### FR-6: Activation condition
- The bulk upload driver is activated when ALL of the following are true:
  1. The remote folder has fewer files than a threshold ratio of the local file count (default: remote has fewer than 10% of local files)
  2. The number of local files pending upload exceeds the file-count threshold (default 50)
  3. No bulk upload is already in progress for the pair
- When the driver is not activated, the standard sync cycle runs normally

### FR-7: Transparent handoff
- After the bulk upload driver completes, the standard sync cycle runs immediately to handle files that changed during the upload or were missed
- The driver's completion is recorded in the activity log
- No user action is required to transition from bulk mode to standard mode

### FR-8: Configuration per sync pair
- `bulk_upload_workers` (integer, default 8, range 1–32): number of parallel upload workers
- `bulk_upload_threshold_files` (integer, default 50): minimum pending file count to trigger bulk mode
- `bulk_upload_chunk_threshold_bytes` (integer, default 10 485 760): file size in bytes above which chunked upload is used
- All three settings are optional; if absent, defaults apply

---

## Success Criteria

1. **Speed**: With 8 workers, uploading 100 files completes in less than one-eighth of the sequential time — confirming near-linear parallelism at this concurrency level
2. **Resume correctness**: After interruption at N completed files, exactly (total − N) upload calls are made on retry; no file is uploaded twice
3. **Large-file chunk resume**: A chunked upload interrupted at chunk N retransmits only chunks N+1 onwards — measured re-transmitted bytes equal (file_size − N × chunk_size) ± one chunk
4. **Progress freshness**: The progress display reflects each completed file within 1 second of its completion during an active bulk upload
5. **Automatic activation**: The bulk upload driver is invoked automatically on first sync of a near-empty remote with ≥ 50 pending local files, with no user configuration required
6. **Transparent handoff**: After bulk upload completes, the standard sync cycle runs successfully and handles any files changed during the upload

---

## Key Entities

### BulkUploadSession (runtime, not persisted)
- `pair_id`: which sync pair this session belongs to
- `total_files`: number of files queued at session start
- `completed_files`: count of successfully uploaded files so far
- `total_bytes`: sum of sizes of all queued files
- `transferred_bytes`: bytes successfully uploaded so far
- `started_at`: wall-clock time the session began
- `status`: queued | uploading | completed | failed

### BulkUploadConfig (persisted per sync pair)
- `workers`: integer 1–32, default 8
- `threshold_files`: integer, default 50
- `chunk_threshold_bytes`: integer, default 10 485 760 (10 MB)

---

## Assumptions

1. The chunked multipart protocol already exists in the codebase and is reused here; no new protocol is implemented.
2. The remote "empty" check uses the remote snapshot already fetched at the start of each sync cycle — no extra network call is needed.
3. The worker pool is in-process (async tasks), not OS threads or separate processes.
4. ETA is computed as: elapsed_time × (total_bytes − transferred_bytes) / transferred_bytes. It resets to "calculating…" on daemon restart.
5. Bandwidth throttling (Feature 009) applies at the transfer layer and interacts transparently with the bulk driver's uploads.
6. Files modified locally during the bulk upload are not detected mid-session; the post-bulk standard sync cycle catches them.

---

## Out of Scope (v1)

- Bulk download
- Differential / delta upload
- Deduplication across pairs or accounts
- Upload scheduling by time of day
- Progress persistence across daemon restarts (file-level resume works, but ETA resets)
- Selective-sync filtering during bulk phase (enforced by the standard cycle after handoff)
