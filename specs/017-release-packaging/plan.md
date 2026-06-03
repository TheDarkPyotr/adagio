# Implementation Plan: Binary Release Packaging

**Branch**: `017-release-packaging` | **Date**: 2026-06-03 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/017-release-packaging/spec.md`

## Summary

Automate binary release packaging for Adagio: a tag-triggered GitHub Actions workflow builds `.deb`, `.rpm`, `.tar.gz`, and `.dmg` artifacts using Tauri's bundler (with `externalBin` for daemon + CLI), generates a changelog via `git-cliff`, attaches SHA-256 checksums, and publishes a GitHub Release. Linux packages include a custom XDG `.desktop` file and icons at all standard sizes.

## Technical Context

**Language/Version**: Rust 1.78 (stable), workspace edition 2021

**Primary Dependencies**:
- Tauri 2 (`tauri build`) — produces `.deb`, `.rpm`, `.AppImage`, `.dmg` from one command
- `git-cliff` — changelog generation from conventional commits
- `softprops/action-gh-release` (GitHub Action) — publish release + attach artifacts
- `sha256sum` / `shasum -a 256` — checksum generation

**Storage**: N/A (CI pipeline artifacts; no persistent storage)

**Testing**: `cargo test --workspace` (existing); smoke test via `dpkg --info` / `rpm -qip` in CI

**Target Platform**: Linux (Ubuntu runner for `.deb`/`.rpm`/`.tar.gz`), macOS (macOS runner for `.dmg`)

**Project Type**: CI/CD pipeline + packaging configuration (no new Rust code)

**Performance Goals**: Full pipeline (test → build → publish) completes in < 30 min (SC-002)

**Constraints**:
- All three components (`adagio-desktop`, `adagio-daemon`, `adagio-cli`) must be in one package (FR-003)
- Version MUST match between `Cargo.toml` workspace version and git tag
- No macOS code signing / notarization in v1

**Scale/Scope**: Single-maintainer release workflow; targets Ubuntu 22.04 and Fedora 40 as primary Linux targets

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Gate | Principle | Status |
|------|-----------|--------|
| Tests authored and FAIL before implementation begins | I. Test-First | ✅ CI smoke tests validate packages before publish step |
| All public Rust items have `///` doc comments | II. Documentation as Code | N/A — no new public Rust items |
| ADR recorded in `docs/adr/` for significant design decisions | II. Documentation as Code | ✅ ADR for `externalBin` bundling strategy added in contracts |
| Structured logging added to all new sync/network operations | III. Observability | N/A — CI pipeline, not runtime code |
| No `println!` in production code paths | III. Observability | N/A |
| New feature implemented as independent crate/module with no direct coupling to core | IV. Extensibility | N/A — CI config only |
| Cross-module calls go through defined trait/interface contracts | IV. Extensibility | N/A |
| Idle memory budget <100 MB RSS confirmed or N/A for this feature | V. Performance-Oriented | N/A |
| UI actions provide feedback within 100 ms confirmed or N/A | V. Performance-Oriented | N/A |
| Benchmarks added for any hot-path changes (sync diff, file I/O, network) | V. Performance-Oriented | N/A |
| `cargo clippy -- -D warnings` passes | Dev Workflow | ✅ Existing CI gate; release workflow inherits |
| `cargo fmt --check` passes | Dev Workflow | ✅ Existing CI gate; release workflow inherits |
| All `unsafe` blocks have `// SAFETY:` comments | Dev Workflow | N/A |
| All three platform CI targets (Linux, macOS, Windows) pass | Technology | ✅ Linux + macOS in release matrix; Windows best-effort |

## Project Structure

### Documentation (this feature)

```text
specs/017-release-packaging/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── contracts/
│   └── release-workflow.md   # Release pipeline contract
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
.github/
└── workflows/
    └── release.yml          # NEW: tag-triggered release pipeline

crates/adagio-desktop/
├── tauri.conf.json          # MODIFY: add icon sizes, externalBin, desktopTemplate
├── icons/                   # MODIFY: add 16/24/48/64/256/512 png refs
└── bundler/
    └── adagio.desktop.template  # NEW: custom XDG desktop entry template

cliff.toml                   # NEW: git-cliff changelog config (repo root)
```

**Structure Decision**: Single workflow file + config changes. No new crates. All changes are in CI config, Tauri config, and asset references.

## Complexity Tracking

> No constitution violations requiring justification.
