# Data Model: CLI Binary (008)

**Date**: 2026-05-30 | **Feature**: `008-cli-binary`

This feature is purely a presentation layer over the existing IPC protocol. It
introduces no new database tables and no new daemon-side data. The types below are
all Rust structs/enums in `adagio-cli` that describe the command-line interface.

---

## CLI argument model (clap derive types)

### Root struct

```rust
#[derive(Parser)]
#[command(name = "adagio", version, about = "Adagio Nextcloud sync client")]
pub struct Cli {
    /// Output results as JSON instead of human-readable text.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}
```

### Top-level subcommands

```rust
#[derive(Subcommand)]
pub enum Commands {
    /// Show sync status for all pairs.
    Status,
    /// Trigger an immediate sync cycle.
    Sync {
        /// Sync only this pair (omit to sync all pairs).
        pair_id: Option<String>,
    },
    /// Pause all sync activity.
    Pause,
    /// Resume sync after pause.
    Resume,
    /// Show recent activity log.
    Activity {
        #[arg(long, default_value = "50")]
        limit: u32,
        #[arg(long, value_parser = ["edit", "sync", "conflict"])]
        filter: Option<String>,
    },
    /// Manage sync pairs.
    Pairs {
        #[command(subcommand)]
        command: PairsCommand,
    },
    /// Manage connected accounts.
    Accounts {
        #[command(subcommand)]
        command: AccountsCommand,
    },
    /// Manage sync conflicts.
    Conflicts {
        #[command(subcommand)]
        command: ConflictsCommand,
    },
    /// Control the background sync daemon.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },
}
```

### Nested subcommand enums

```rust
#[derive(Subcommand)]
pub enum PairsCommand {
    /// List all configured sync pairs.
    List,
    /// Add a new sync pair.
    Add {
        #[arg(long)]
        local: String,
        #[arg(long)]
        remote: String,
        #[arg(long)]
        account: String,
    },
    /// Remove a sync pair.
    Remove {
        pair_id: String,
        /// Also delete all local files in the synced folder.
        #[arg(long)]
        delete_local_files: bool,
    },
}

#[derive(Subcommand)]
pub enum AccountsCommand {
    /// List all connected accounts.
    List,
    /// Remove an account (and all its pairs).
    Remove { account_id: String },
}

#[derive(Subcommand)]
pub enum ConflictsCommand {
    /// List pending conflicts.
    List {
        /// Show only conflicts for this pair.
        pair_id: Option<String>,
    },
    /// Resolve a conflict.
    Resolve {
        conflict_id: String,
        /// Which version to keep: local, remote, or both.
        #[arg(long, value_parser = ["local", "remote", "both"])]
        keep: String,
    },
    /// Dismiss all conflicts without file I/O.
    Dismiss,
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Start the daemon if not already running.
    Start,
    /// Stop the daemon gracefully.
    Stop,
    /// Show daemon status and uptime.
    Status,
}
```

---

## Exit code type

```rust
pub enum ExitCode {
    Success   = 0,  // operation completed normally
    UsageError = 1, // bad arguments (clap handles automatically)
    DaemonErr  = 2, // daemon returned an application-level error
    Unreachable = 3, // daemon not reachable and could not be started
}
```

---

## Output shapes (printed to stdout)

All commands print one of two forms depending on `--json`:

### Human mode

Plain text; columns aligned with `format!("{:<N}")`. Colours are intentionally
omitted so output is safe to redirect to files and piped to `grep`.

### JSON mode

`serde_json::to_string_pretty(value)` of the raw `serde_json::Value` returned by
`DaemonClient::request()`. No additional transformation is applied — the shape is
whatever the daemon returns (see `contracts/ipc-protocol.md` in feature 007 for
the daemon's response shapes).

---

## No new database entities

The CLI is stateless. It reads and writes data exclusively via IPC to the daemon.
The daemon owns all persistent state.
