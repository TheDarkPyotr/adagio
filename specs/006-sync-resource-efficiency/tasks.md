# Tasks: Sync Resource Efficiency

**Input**: Design documents from `specs/006-sync-resource-efficiency/`

**Prerequisites**: plan.md ✅ | spec.md ✅ | research.md ✅ | data-model.md ✅ | contracts/internal-module.md ✅

**Tests**: Per Constitution Principle I (Test-First, NON-NEGOTIABLE), test tasks are
MANDATORY for all implementation work. Tests MUST be written first and MUST FAIL before
implementation begins.

**Format**: `[ID] [P?] [Story?] Description`
- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[US1]**: Belongs to User Story 1 (the only story in this feature)

---

## Phase 1: Setup

**Purpose**: ADRs documenting the two significant design decisions before any code changes.

- [x] T001 Create `docs/adr/007-missed-tick-skip.md` — document decision to use `MissedTickBehavior::Skip` over default `Burst`; describe the catch-up-storm failure mode and why `Delay` was also rejected
- [x] T002 [P] Create `docs/adr/008-bounded-conflict-channel.md` — document decision to change `UnboundedSender<()>` to bounded `Sender<()>(1)`; explain why capacity-1 is sufficient and how `TrySendError::Full` is handled

---

## Phase 2: Foundational

**Purpose**: All four acceptance criteria are independent — no shared blocking prerequisite
beyond the ADRs above. There are no schema migrations or new Tauri commands.

**Checkpoint**: ADRs written. Four independent AC tracks can now proceed in parallel.

---

## Phase 3: User Story 1 — Reliable Sync That Stays Out of the Way (Priority: P1) 🎯

**Goal**: All four acceptance criteria pass: CPU ≤ 1% at idle, no file locks, RSS growth
≤ 20% in 24 h, ≤ 3 retries/second per path.

**Independent Test**: Run the client against a fully-synced pair for 24 hours; verify
CPU/RSS/retry metrics from logs and `cargo bench`. Each AC can be validated independently
without the others (see `quickstart.md`).

### Tests for User Story 1 (write first — must FAIL)

- [x] T003 [P] [US1] Write failing Rust unit test `missed_tick_skip_does_not_fire_twice_after_slow_cycle` in `crates/adagio-core/src/cycle/runner.rs` test module — use `tokio::time::pause()` + `advance(2 × interval)`, assert ticker fires exactly once
- [x] T004 [P] [US1] Write failing Rust unit test `cycle_metrics_reports_zero_recomputed_for_unchanged_files` in `crates/adagio-core/src/cycle/mod.rs` test module — run scanner twice on the same dir, assert second `ScanStats.checksum_recomputed == 0`
- [x] T005 [P] [US1] Write failing Rust unit test `scan_concurrent_open_file_no_lock` in `crates/adagio-core/src/detection/local.rs` test module — hold `File::open` in a thread while `scan_blocking()` runs; assert scan returns `Ok(...)` with no `PermissionDenied` or `WouldBlock`
- [x] T006 [P] [US1] Write failing Rust unit tests in `crates/adagio-core/src/observability.rs` test module:
  - `memory_sampler_returns_nonzero_rss` — call `MemorySampler::sample_rss()`, assert `Some(n > 0)`
  - `memory_sampler_warn_on_growth_above_threshold` — inject fake RSS 116 MB vs 100 MB baseline; assert `growth_ratio == Some(1.16)` and WARN event fired
  - `conflict_channel_full_does_not_block_propagator` — fill bounded channel (cap 1), call `try_send(())`, assert `Err(TrySendError::Full)`
  - `memory_stable_over_many_cycles` — run 500 mock cycles; assert final RSS < 1.20 × initial
- [x] T007 [P] [US1] Write failing Rust unit tests in `crates/adagio-core/src/cycle/propagator.rs` test module:
  - `retry_log_contains_attempt_and_delay_fields` — simulate 3 transient errors; assert captured tracing events carry `retry_attempt` 0/1/2 and `delay_ms >= 1000`
  - `retry_rate_never_exceeds_3_per_second` — record `Instant` of each mock call over 5 retries; assert every consecutive pair ≥ 333 ms apart

### Implementation for User Story 1

**AC-1: CPU idle behaviour**

- [x] T008 [P] [US1] Add `MissedTickBehavior::Skip` to the interval in `crates/adagio-core/src/cycle/runner.rs` — one line after `tokio::time::interval(Duration::from_secs(interval_secs))`; add `///` doc comment on `PairRunner::spawn` explaining the skip semantics
- [x] T009 [P] [US1] Add `ScanStats { checksum_recomputed: usize, checksum_from_cache: usize }` to `crates/adagio-core/src/detection/local.rs`; thread counters through `scan_blocking()` (increment `checksum_from_cache` at line 174, `checksum_recomputed` at lines 176/179); update `scan_blocking()` return type to `(Vec<LocalEntry>, ScanStats)` and add `///` doc comment to `compute_checksum()` documenting non-exclusive sharing semantics (covers AC-2)
- [x] T010 [US1] Add `CycleMetrics` struct to `crates/adagio-core/src/observability.rs` (fields: `path_count`, `checksum_recomputed`, `checksum_from_cache`, `duration_ms`); update `crates/adagio-core/src/cycle/mod.rs` and any intermediate callers (`reconciler.rs`) to unwrap the new `(result, ScanStats)` tuple from `scan_blocking()` and emit `tracing::debug!(path_count, checksum_recomputed, checksum_from_cache, duration_ms, "cycle complete")` at cycle end — depends on T009
- [x] T011 [P] [US1] Create `crates/adagio-core/benches/idle_scan.rs` — criterion benchmark over a 1,000-file temp dir; measure second (steady-state) run duration; assert `stats.checksum_recomputed == 0`; add `[[bench]]` entry to `crates/adagio-core/Cargo.toml`

**AC-3: Memory stability**

- [x] T012 [US1] Add `MemorySampler` and `MemorySample` to `crates/adagio-core/src/observability.rs`: `new()`, `set_baseline(rss: u64)` (panics on second call), `sample_rss() -> Option<u64>` (platform-specific: Linux `/proc/self/statm`, macOS `RUSAGE_SELF`, Windows `GetProcessMemoryInfo`; `unsafe` block with `// SAFETY:` comment on Linux), `observe(&self) -> MemorySample`; `observe()` emits `tracing::debug!` always, `WARN` at growth > 15%, `ERROR` at growth > 20%; all public items carry `///` doc comments — depends on T010 (same file, avoid conflict)
- [x] T013 [US1] Integrate `MemorySampler` into `crates/adagio-core/src/cycle/mod.rs`: add field to `DefaultSyncEngine`, call `sampler.observe()` at end of each cycle in `SyncCycle::run`, call `set_baseline()` after the first successful cycle completes — depends on T012
- [x] T014 [P] [US1] Change `mpsc::unbounded_channel::<()>()` → `mpsc::channel::<()>(1)` in `crates/adagio-desktop/src/lib.rs`; update the conflict-notification send site in `crates/adagio-core/src/cycle/propagator.rs` from `.send(()).await` → `.try_send(())` with `TrySendError::Full` arm silently dropped; update `SyncCycle.conflict_tx` field type from `Option<&'a UnboundedSender<()>>` to `Option<&'a Sender<()>>` in `crates/adagio-core/src/cycle/mod.rs`

**AC-4: Retry observability**

- [x] T015 [P] [US1] In `crates/adagio-core/src/cycle/propagator.rs` `with_retry()`: replace existing `tracing::warn!(attempt, "transient error; retrying")` with `tracing::warn!(retry_attempt = attempt, delay_ms = delay.as_millis() as u64, "transient error — retrying after backoff")`; no other changes to `with_retry` — can be done concurrently with T014 if T014 does not touch the same `warn!` callsite (it does not)

**Checkpoint**: All four acceptance criteria implemented. All failing tests from T003–T007 should now pass.

---

## Phase 4: Polish & Cross-Cutting Concerns

- [x] T016 `cargo clippy -- -D warnings` — fix all warnings across `adagio-core` and `adagio-desktop`; zero suppressions
- [x] T017 [P] `cargo fmt --all --check` — run `cargo fmt --all` to fix any formatting drift introduced during implementation
- [x] T018 `cargo test --lib -p adagio-core` — all Rust unit tests pass (125 existing + ~10 new)
- [x] T019 `cargo bench -p adagio-core -- idle_scan` — benchmark runs without panic; note baseline result in PR description
- [ ] T020 Run `specs/006-sync-resource-efficiency/quickstart.md` end-to-end validation — manual CPU spot-check, file-lock check, RSS baseline vs 24-h measurement, retry log audit; confirm all four ACs pass; update `quickstart.md` if any step is inaccurate

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: No blocking prerequisites beyond Phase 1
- **Phase 3 (US1)**: Depends on Phase 1; all four AC tracks are independent of each other
- **Phase 4 (Polish)**: Depends on all Phase 3 tasks

### Within Phase 3 (US1)

- All test tasks (T003–T007) are [P] — write all failing tests first, before any implementation
- T009 before T010 (T010 consumes the `ScanStats` tuple added by T009)
- T010 before T012 (both modify `observability.rs`; T012 adds to what T010 starts)
- T012 before T013 (`MemorySampler` must exist before it's wired into the cycle)
- T008, T011 are fully independent (different files)
- T014 and T015 both touch `propagator.rs` — sequence T014 before T015 to avoid conflicts

### Parallel Opportunities

All test tasks (T003–T007) can be written simultaneously — different test modules.

After tests are written and failing:
- T008 (runner.rs) — fully independent
- T009 (local.rs) — fully independent
- T011 (benches/) — fully independent
- T014 + T015 (propagator.rs + lib.rs) — T014 first, then T015
- T010 → T012 → T013 (observability.rs + mod.rs chain) — sequential

---

## Parallel Example: Phase 3 Test Sprint

```
# Write all failing tests simultaneously before any implementation:
T003: missed_tick_skip_*                  → crates/adagio-core/src/cycle/runner.rs
T004: cycle_metrics_*                     → crates/adagio-core/src/cycle/mod.rs
T005: scan_concurrent_open_file_no_lock   → crates/adagio-core/src/detection/local.rs
T006: memory_sampler_* (4 tests)          → crates/adagio-core/src/observability.rs
T007: retry_log_* (2 tests)               → crates/adagio-core/src/cycle/propagator.rs

# Then implement in parallel where files don't overlap:
T008 (runner.rs) ──────────────────────── independent
T009 (local.rs) ───────────────────────── independent
T011 (benches/idle_scan.rs) ───────────── independent
T009 → T010 → T012 → T013 (mod.rs chain) sequential
T014 → T015 (propagator.rs) ───────────── sequential
```

---

## Implementation Strategy

### MVP (All ACs in one story)

1. Phase 1 → Phase 2 checkpoint
2. Write all failing tests (T003–T007) simultaneously
3. Implement in dependency order: T008, T009 in parallel → T010 → T011 in parallel → T012 → T013 → T014 → T015
4. **STOP and VALIDATE**: `cargo test --lib` all green; `cargo bench` runs
5. Phase 4: Polish + quickstart validation

### Notes

- [P] tasks touch different files — launch together for speed
- T009 is the keystone of the observability chain — do it early; all downstream tasks (T010, T012, T013) depend on its `ScanStats` type being defined
- T014 (channel type change) is a breaking change across two crates — compile after T014 before starting T015 to catch type errors early
- The `unsafe` block in `MemorySampler::sample_rss` (Linux `/proc/self/statm` read) requires a `// SAFETY:` comment per Constitution §Dev Workflow — include it in T012
