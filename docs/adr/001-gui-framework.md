# ADR 001: GUI Framework — Tauri 2.x + Svelte

**Status**: Accepted  
**Date**: 2025-05-24

## Context

Adagio is a cross-platform Nextcloud desktop client. We need a GUI framework that:

- Runs on Linux, macOS, and Windows from a single codebase
- Integrates with our Rust core library without an FFI boundary
- Produces a small binary with low idle-memory footprint (< 100 MB RSS per constitution Principle V)
- Supports modern, reactive UI patterns

## Decision

Use **Tauri 2.x** as the application shell and **Svelte** as the UI framework.

Tauri uses the platform's native WebView (WebKitGTK on Linux, WKWebView on macOS, WebView2 on Windows), eliminating the need to bundle Chromium and keeping binary size and memory usage low. The Rust backend communicates with the frontend via typed `#[tauri::command]` IPC, keeping the entire sync engine and journal in native Rust with zero FFI overhead.

Svelte compiles to vanilla JavaScript with no virtual DOM runtime, aligning with our lean memory goal.

## Alternatives Considered

| Framework | Rejected because |
|-----------|-----------------|
| Electron + React | Bundles Chromium (~200 MB); > 300 MB RSS at idle |
| iced (pure Rust GUI) | Immature ecosystem; no web-component story for future cloud panels |
| Qt (via cxx-qt) | Complex FFI; C++ build toolchain dependency; licensing concerns |
| Flutter Desktop | Dart FFI to Rust adds complexity; larger binary |

## Consequences

- UI code is TypeScript/Svelte (not Rust); contributors need frontend skills
- Platform-specific WebView versions may cause rendering inconsistencies
- Tauri's async IPC is the API surface between core and UI; all commands are explicitly registered
