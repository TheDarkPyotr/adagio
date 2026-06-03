# ADR-012: Migrate platform_config_dir() to adagio-ipc::transport

**Date**: 2026-05-30
**Status**: Accepted
**Feature**: 008-cli-binary

## Context

`platform_config_dir()` computes the platform-specific path for Adagio's config and
database directory (XDG data home on Linux, Application Support on macOS, AppData on
Windows). It was originally written inline in `adagio-daemon/src/main.rs`.

The CLI (`adagio-cli`) must pass the same path as `--config-dir` when spawning a new
daemon. Both binaries need the same logic.

## Decision

Move `platform_config_dir()` to `adagio-ipc/src/transport.rs` as a `pub fn`. Both
`adagio-daemon` and `adagio-cli` call `adagio_ipc::transport::platform_config_dir()`.

## Rationale

`adagio-ipc` is already a dependency of both crates. The function belongs alongside
`daemon_socket_path()` and `ensure_socket_dir()` — all three are platform-specific
path helpers for the daemon IPC setup. Duplicating the logic in `adagio-cli` would
risk silent divergence (e.g., Linux uses data home vs. config home).

## Alternatives Considered

**Duplicate in adagio-cli**: Quick but fragile — any platform bug fix must be applied
in two places.

**Move to a new `adagio-platform` crate**: Over-engineering; the function is 10 lines.
`adagio-ipc::transport` is already the right home for platform-specific path logic.
