# ADR-009: NDJSON over Unix Socket / Named Pipe for IPC

**Date**: 2026-05-29
**Status**: Accepted
**Feature**: 007-background-sync-daemon

## Context

The `adagio-daemon` process needs to expose a local IPC endpoint so `adagio-desktop`
(and a future CLI) can send commands and receive push events. The transport must be
fast (local loopback), secure (OS-enforced user isolation), and cross-platform.

## Decision

Use **newline-delimited JSON (NDJSON)** framed over:
- `tokio::net::UnixListener` (Linux, macOS)
- `tokio::net::windows::named_pipe::ServerOptions` (Windows)

Both are built into tokio's `full` feature, which the workspace already uses. No
additional crate is required.

**Two connection types** on the same socket/pipe endpoint:
1. **RPC connection** — identified by the first message not being `{"type":"subscribe"}`.
   Each line is a JSON-RPC 2.0 request; the daemon responds with a matching `id`.
2. **Subscription connection** — identified by the first message being
   `{"type":"subscribe"}`. After that, the daemon pushes NDJSON event lines
   indefinitely; the client only reads.

**Framing**: newline (`\n`) as the frame delimiter. No length prefix is needed
because JSON objects cannot contain bare newlines.

**Security**: Unix socket created with `0600` permissions (owner read/write only).
Named pipe created with a user-SID-scoped path to prevent cross-user access on
Windows.

## Alternatives Considered

**jsonrpc-core**: Unmaintained since 2022. Avoid for new code.

**jsonrpsee**: Actively maintained but has no built-in Unix socket transport; a
custom `AsyncRead`/`AsyncWrite` adapter would add more code than the hand-rolled
approach and pulls in a large dependency graph.

**tarpc**: Good for async trait RPC but is not a line-protocol JSON-RPC API.
Schema mismatch with the existing Tauri DTO shapes would require extra translation.

**gRPC / tonic**: The proposal's long-term target. Deferred to a future feature to
avoid introducing the protobuf toolchain in this extraction step. The NDJSON
protocol is designed to be drop-in replaceable with gRPC in a later iteration.

## Consequences

- No new crate dependencies for the transport layer.
- The protocol is human-readable and debuggable with `nc` / `netcat`.
- Each RPC method maps 1:1 to an existing Tauri command handler body — copy + adapt.
- The subscription channel allows real-time UI updates without polling.
