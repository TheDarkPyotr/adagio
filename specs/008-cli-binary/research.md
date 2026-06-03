# Research: CLI Binary (008)

**Date**: 2026-05-30 | **Feature**: `008-cli-binary`

---

## Decision 1 — Argument parsing: clap v4 with derive

**Decision**: Add `clap = { version = "4.5", features = ["derive"] }` to
`[workspace.dependencies]`. Use `#[derive(Parser, Subcommand, Args)]` throughout.
Global `--json` flag declared with `#[arg(global = true)]` on the root `Cli` struct —
clap propagates it to every subcommand automatically without any extra code.

**Rationale**: `clap` is not currently in the workspace. It is the de-facto standard
for Rust CLIs, handles nested subcommands (`adagio daemon status`, `adagio conflicts
resolve ID --keep local`) cleanly via enum variants, and renders `[PAIR_ID]` help
text automatically for `Option<String>` positionals. `global = true` solves the
`--json` propagation problem with one attribute.

**Alternatives considered**:
- `argh` — faster compile times, smaller binary, but lacks global flags and has a
  more rigid help format.
- `structopt` — superseded by clap v4.

---

## Decision 2 — Output formatting: hand-rolled with format! padding (no table crate)

**Decision**: No additional crate for table rendering. Use `format!("{:<width$}",
value, width = N)` for human-readable column alignment. JSON mode prints the raw
`serde_json::Value` via `serde_json::to_string_pretty`.

**Rationale**: All data comes from the daemon already as `serde_json::Value`. A table
crate (`tabled`, `comfy-table`) would need the data re-structured into typed rows
before rendering, adding complexity without proportional benefit. The tables are
simple (3–5 columns, no cell merging, no borders needed beyond header underline).
Hand-rolling is 15–20 lines of code per table and adds zero binary size.

**Alternatives considered**:
- `tabled v0.17` — native JSON support but adds ~2MB to binary; overkill for this.
- `comfy-table` — excellent ANSI styling but data must be re-cast into `Cell` types.

---

## Decision 3 — New crate: adagio-cli

**Decision**: Add `crates/adagio-cli/` to the workspace as a binary crate. It
depends on `adagio-ipc` (for `DaemonClient` and `DaemonRequest`) and `clap`.
It does NOT depend on `adagio-core`, `adagio-nextcloud`, or `adagio-desktop`.

**Rationale**: Keeping the CLI in its own crate enforces the architectural constraint
that it is purely an IPC client. It also keeps the binary small — the daemon's
heavyweight dependencies (tokio-full, sqlx, reqwest) are not linked into the CLI.

**Crate dependency graph**:
```
adagio-cli → adagio-ipc → (tokio, serde_json, tracing)
adagio-cli → clap
```

---

## Decision 4 — DaemonClient connection

**Decision**: Call `DaemonClient::connect_or_start(daemon_binary_path)` at the start
of every command that requires the daemon. The daemon binary path is derived as a
sibling of the CLI executable: `std::env::current_exe()?.parent()?.join("adagio-daemon")`.

**Rationale**: `connect_or_start` already implements the fast-path (daemon running →
connect) and slow-path (not running → spawn + retry for 5 s). The CLI gets full
auto-start behavior for free. This is the same pattern used by `adagio-desktop`'s
`lifecycle.rs`.

**Commands that do NOT connect to daemon**: `daemon start` (it IS the start trigger;
but `connect_or_start` handles this), `--help`, `--version`. Even `daemon start`
uses `connect_or_start` since that function spawns when the daemon is absent.

---

## Decision 5 — platform_config_dir() moved to adagio-ipc::transport

**Decision**: Move `platform_config_dir()` from `adagio-daemon/src/main.rs` into
`adagio-ipc/src/transport.rs` and re-export it. Both `adagio-daemon` and `adagio-cli`
call it to determine the config directory path passed as `--config-dir` when spawning
the daemon.

**Rationale**: The CLI needs the same logic as the daemon to know where to pass as
`--config-dir`. Duplicating it would risk divergence. Putting it in `adagio-ipc`
(which both already depend on) is the natural home — it belongs alongside
`daemon_socket_path()` and `ensure_socket_dir()`.

---

## Decision 6 — Exit code implementation

**Decision**: Use `std::process::exit(code)` at the very end of `main()` after
all output is flushed. Exit codes: **0** success, **1** usage/argument error
(clap handles this automatically), **2** daemon returned an application-level error,
**3** daemon not reachable (connection failed or start failed).

**Rationale**: Clap already calls `process::exit(1)` for argument parse errors, so
the CLI only needs to handle codes 2 and 3. Mapping daemon `anyhow::Error` types to
these codes is done via a thin `run()` function that returns `Result<(), CliError>`.

---

## Decision 7 — DaemonRequest coverage

The CLI implements 22 of 24 `DaemonRequest` variants, covering all user-facing
operations. Two variants are intentionally excluded:

| Excluded variant | Reason |
|---|---|
| `ConnectAccountOAuth2` | Requires browser-based OAuth2 flow — CLI cannot open a browser |
| `Subscribe` | Internal subscription marker sent on the events connection — not a user command |

All pairs, accounts, sync control, conflicts, activity, and daemon lifecycle commands
map 1:1 to existing `DaemonRequest` variants.
