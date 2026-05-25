<!-- Sync Impact Report
Version change: N/A → 1.0.0 (initial ratification)
Modified principles: None (first version)
Added sections: Core Principles (I–V), Technology & Architecture, Development Workflow, Governance
Removed sections: None
Templates requiring updates:
  - .specify/templates/plan-template.md ✅ Constitution Check gates updated
  - .specify/templates/spec-template.md ✅ No structural changes required
  - .specify/templates/tasks-template.md ✅ Test-First enforcement comment updated
Deferred TODOs: None
-->

# Adagio Constitution

## Core Principles

### I. Test-First (NON-NEGOTIABLE)

Tests MUST be written before any implementation code. The Red-Green-Refactor cycle is
strictly enforced across all feature work:

- Tests MUST be authored and reviewed to FAIL before any implementation begins.
- No implementation task is complete unless all associated tests pass.
- Unit, integration, and end-to-end tests MUST cover all public API surfaces.
- Contract tests are REQUIRED at every cross-module boundary.

**Rationale**: Catching design flaws during test authoring is cheaper than during
integration and ensures every change ships with a regression harness from day one.

### II. Documentation as Code

Every public API, module entry point, and behavioral contract MUST be documented in the
source tree — documentation is a first-class deliverable, not an afterthought.

- Public Rust items (types, traits, functions) MUST carry `///` doc comments.
- Architectural decisions MUST be recorded in `docs/adr/` as Architecture Decision Records.
- The `specs/` directory MUST remain in sync with the implemented feature set.
- Undocumented public interfaces MUST NOT be merged to `main`.

**Rationale**: A desktop client integrating multiple Nextcloud services will grow complex
over time; documentation prevents knowledge silos and reduces onboarding friction.

### III. Observability

All runtime state MUST be inspectable without attaching a debugger.

- Structured logging (JSON or key=value format) is REQUIRED throughout the application.
- Log levels (ERROR, WARN, INFO, DEBUG, TRACE) MUST be applied consistently;
  bare `println!` calls MUST NOT appear in production code.
- Sync operations and all network calls MUST emit timing metrics.
- Background tasks MUST log start, completion, and failure with sufficient context
  to diagnose the issue without reproducing it.

**Rationale**: A sync client runs long-lived background operations; silent failures destroy
user trust. Structured logs make production diagnosis tractable.

### IV. Extensibility

The architecture MUST support adding new Nextcloud application integrations (Notes, Talk,
Calendar, and future apps) without modifying the core sync infrastructure.

- The core sync engine MUST be decoupled from feature modules via stable internal
  trait/interface boundaries.
- Each Nextcloud integration MUST be implemented as an independent crate or feature module.
- Public extension APIs MUST be versioned following the project versioning policy.
- Tight coupling between feature modules is PROHIBITED; all cross-module calls MUST
  go through defined contracts.

**Rationale**: Nextcloud's app ecosystem is large and evolving; tight coupling would make
each new integration a rework of prior infrastructure.

### V. Performance-Oriented

Resource efficiency is a first-class constraint, not an optimization pass left for later.

- **Memory**: The idle client MUST consume less than 100 MB RSS on a clean sync state.
- **CPU**: Background sync MUST NOT exceed 5% CPU on a modern laptop core during
  steady-state operation.
- **UI responsiveness**: All user-initiated actions MUST complete or provide visible
  feedback within 100 ms.
- Performance regressions MUST be caught before merge; benchmarks are REQUIRED for
  hot paths (sync diffing, file I/O, network throughput).

**Rationale**: Adagio targets developer workstations where competing sync clients are
known to be heavy; lightweight performance is the core differentiator.

## Technology & Architecture

- **Language**: Rust (stable toolchain, edition 2021 minimum).
- **UI Layer**: Cross-platform GUI framework (Tauri or equivalent); native OS integration
  APIs MUST be preferred over emulation layers where available.
- **Async Runtime**: Tokio; blocking I/O MUST NOT execute on async executor threads.
- **Crate structure**: Cargo workspace layout — `crates/core` for the sync engine,
  `crates/ui` for the desktop shell, one crate per Nextcloud feature integration.
- **Platform targets**: Linux, macOS, and Windows. All three MUST pass CI on every PR.
- **Unsafe code policy**: `unsafe` blocks MUST include a `// SAFETY:` comment explaining
  the invariant upheld, and MUST receive heightened review scrutiny before merge.

## Development Workflow

- **Branching**: Feature branches are created from `main`; naming follows the Specify
  convention (`###-feature-name`).
- **Pull Requests**: Every PR MUST reference a spec and MUST pass all CI quality gates
  before merge.
- **Code Review**: At least one approval is required; reviewers MUST verify Constitution
  compliance as an explicit part of the review checklist.
- **Quality Gates** (all MUST pass before merge):
  1. All tests pass (unit + integration + contract).
  2. `cargo clippy -- -D warnings` passes with no suppressions.
  3. `cargo fmt --check` passes.
  4. No new `unsafe` blocks without an accompanying `// SAFETY:` comment.
  5. All new public API items include doc comments.
  6. No performance-budget violations on hot-path changes (benchmark gate).

## Governance

This Constitution supersedes all other project practices and conventions. It is the
authoritative source for engineering decisions; where any documentation conflicts with
the Constitution, the Constitution prevails.

- **Amendments**: Any principle change requires a PR that (a) updates this file,
  (b) updates all affected templates, (c) describes the migration path for existing code,
  and (d) receives explicit approval from the project lead.
- **Versioning policy**: MAJOR for backward-incompatible governance changes or principle
  removals; MINOR for new principles or materially expanded guidance; PATCH for wording
  and clarifications only.
- **Compliance review**: All PR reviews MUST include a Constitution Check. The active
  gate list is maintained in `.specify/templates/plan-template.md`.
- **Guidance file**: Use `CLAUDE.md` for runtime development guidance specific to the AI
  toolchain.

**Version**: 1.0.0 | **Ratified**: 2026-05-24 | **Last Amended**: 2026-05-24
