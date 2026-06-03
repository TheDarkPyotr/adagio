# ADR 017: Release Packaging Strategy

**Status**: Accepted
**Date**: 2026-06-03
**Feature**: 017-release-packaging

## Context

Adagio needs a release pipeline that builds installable binaries for Linux (`.deb`, `.rpm`, `.tar.gz`) and macOS (`.dmg`), all containing three components: `adagio-desktop`, `adagio-daemon`, and `adagio-cli`.

## Decision

Use **Tauri's built-in bundler** (`tauri build`) as the sole packaging tool, with `bundle.externalBin` to co-package `adagio-daemon` and `adagio-cli` alongside the desktop app.

A single **GitHub Actions workflow** (`release.yml`) triggers on semver tags, runs the full test suite as a gate, builds platform artifacts in parallel (Linux + macOS), and publishes via `softprops/action-gh-release`.

Changelog generation uses **`git-cliff`** with conventional commit parsing.

## Alternatives Considered

### Option A: Separate packages per component
Rejected — requires users to install 3 separate packages and manage inter-package dependencies; worse UX.

### Option B: Custom `fpm`-based packaging in Docker
Rejected — `fpm` adds complexity and a Docker dependency for CI. Tauri already produces `.deb` and `.rpm` natively with correct metadata from `Cargo.toml`.

### Option C: Use `goreleaser`
Rejected — `goreleaser` is excellent but Go-native. The project is Rust/Tauri; using the Tauri CLI is more natural and avoids a cross-ecosystem tool.

### Option D: Use `tauri-apps/tauri-action` (official GitHub Action)
Not chosen as the primary mechanism — the official Tauri action handles signing/notarization which is out of scope for v1. A manual workflow gives more explicit control over the test gate and artifact naming.

## Consequences

- One installer per platform: users install Adagio once and get all three binaries.
- The `binaries/` staging directory in `crates/adagio-desktop/` is a build-time artifact — it is `.gitignore`'d and populated by CI before `tauri build`.
- Tauri `externalBin` requires platform-suffixed binary names at build time; the CI `Stage external binaries` step handles this using `rustc -vV` to get the host triple.
- macOS code signing and notarization are deferred to a future release (Gatekeeper warning documented in release notes).
