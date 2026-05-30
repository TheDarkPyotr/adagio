use clap::{Parser, Subcommand};

/// Adagio sync client — command-line interface.
///
/// Connects to the running `adagio-daemon` and sends a single command, then exits.
/// If the daemon is not running, it is started automatically.
#[derive(Parser, Debug)]
#[command(name = "adagio", version, about = "Adagio Nextcloud sync client")]
pub struct Cli {
    /// Output results as JSON instead of human-readable text.
    ///
    /// When set, stdout contains only valid JSON. Errors go to stderr.
    #[arg(long, global = true)]
    pub json: bool,

    #[command(subcommand)]
    pub command: Commands,
}

/// Top-level subcommands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Show sync status for all configured pairs.
    Status,
    /// Trigger an immediate sync cycle (all pairs, or one specific pair).
    Sync {
        /// Sync only this pair ID. Omit to sync all pairs.
        pair_id: Option<String>,
    },
    /// Pause all sync activity.
    Pause,
    /// Resume sync after pause.
    Resume,
    /// Show recent sync activity log.
    Activity {
        /// Maximum number of entries to show.
        #[arg(long, default_value = "50")]
        limit: u32,
        /// Filter by kind: edit, sync, or conflict.
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
    /// Manage bandwidth limits for sync transfers.
    Bandwidth {
        #[command(subcommand)]
        command: BandwidthCommand,
    },
    /// Manage network-awareness policy (metered, battery, SSID blocking).
    Network {
        #[command(subcommand)]
        command: NetworkCommand,
    },
}

/// Subcommands for `adagio network`.
#[derive(Subcommand, Debug)]
pub enum NetworkCommand {
    /// Show current network state and active policy.
    Status,
    /// Configure network-awareness policy.
    Set {
        /// Action when on a metered connection: allow, throttle, or pause.
        #[arg(long, value_parser = ["allow", "throttle", "pause"])]
        on_metered: Option<String>,
        /// Action when on battery power: allow, throttle, or pause.
        #[arg(long, value_parser = ["allow", "throttle", "pause"])]
        on_battery: Option<String>,
        /// Shared throttle limit in Kbps (used when on-metered or on-battery is throttle).
        #[arg(long)]
        throttle_kbps: Option<u64>,
    },
    /// Add an SSID to the block list (sync pauses when connected to it).
    BlockSsid {
        /// The exact Wi-Fi network name to block (case-sensitive).
        ssid: String,
    },
    /// Remove an SSID from the block list.
    UnblockSsid {
        /// The SSID to unblock.
        ssid: String,
    },
    /// List all blocked SSIDs.
    ListBlocked,
}

/// Subcommands for `adagio bandwidth`.
#[derive(Subcommand, Debug)]
pub enum BandwidthCommand {
    /// Show current bandwidth limits and measured throughput.
    Status,
    /// Set upload and/or download speed limits (Kbps; 0 = unlimited).
    Set {
        /// Upload limit in Kbps (omit to keep current value).
        #[arg(long)]
        upload_kbps: Option<u64>,
        /// Download limit in Kbps (omit to keep current value).
        #[arg(long)]
        download_kbps: Option<u64>,
    },
    /// Remove all bandwidth limits.
    Clear,
}

/// Subcommands for `adagio pairs`.
#[derive(Subcommand, Debug)]
pub enum PairsCommand {
    /// List all configured sync pairs.
    List,
    /// Add a new sync pair.
    Add {
        /// Local folder to sync.
        #[arg(long)]
        local: String,
        /// Remote Nextcloud folder path.
        #[arg(long)]
        remote: String,
        /// Account ID to use for this pair.
        #[arg(long)]
        account: String,
    },
    /// Remove a sync pair.
    Remove {
        /// Pair ID to remove.
        pair_id: String,
        /// Also delete all locally synced files.
        #[arg(long)]
        delete_local_files: bool,
    },
}

/// Subcommands for `adagio accounts`.
#[derive(Subcommand, Debug)]
pub enum AccountsCommand {
    /// List all connected Nextcloud accounts.
    List,
    /// Remove an account and all of its sync pairs.
    Remove {
        /// Account ID to remove.
        account_id: String,
    },
}

/// Subcommands for `adagio conflicts`.
#[derive(Subcommand, Debug)]
pub enum ConflictsCommand {
    /// List pending (unresolved) conflicts.
    List {
        /// Show only conflicts for this pair ID.
        pair_id: Option<String>,
    },
    /// Resolve a conflict by choosing which version to keep.
    Resolve {
        /// Conflict ID to resolve.
        conflict_id: String,
        /// Which version to keep: local, remote, or both.
        #[arg(long, value_parser = ["local", "remote", "both"])]
        keep: String,
    },
    /// Dismiss all pending conflicts without any file I/O.
    Dismiss,
}

/// Subcommands for `adagio daemon`.
#[derive(Subcommand, Debug)]
pub enum DaemonCommand {
    /// Start the background daemon (if not already running).
    Start,
    /// Stop the daemon gracefully (waits up to 30 s for transfers to complete).
    Stop,
    /// Show the daemon's current status, uptime, and version.
    Status,
}

#[cfg(test)]
mod tests {
    use super::*;

    // T006-a — adagio status is parsed correctly.
    #[test]
    fn cli_parses_status_command() {
        let cli = Cli::parse_from(["adagio", "status"]);
        assert!(matches!(cli.command, Commands::Status));
        assert!(!cli.json);
    }

    // T006-b — --json before subcommand sets json=true.
    #[test]
    fn cli_global_json_flag() {
        let cli = Cli::parse_from(["adagio", "--json", "status"]);
        assert!(cli.json);
    }

    // T006-c — --json after subcommand also sets json=true (global flag).
    #[test]
    fn cli_json_after_subcommand() {
        let cli = Cli::parse_from(["adagio", "status", "--json"]);
        assert!(cli.json);
    }

    // T006-d — conflicts resolve parses ID and --keep.
    #[test]
    fn cli_parses_conflicts_resolve() {
        let cli = Cli::parse_from([
            "adagio",
            "conflicts",
            "resolve",
            "abc-123",
            "--keep",
            "local",
        ]);
        match cli.command {
            Commands::Conflicts {
                command: ConflictsCommand::Resolve { conflict_id, keep },
            } => {
                assert_eq!(conflict_id, "abc-123");
                assert_eq!(keep, "local");
            }
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn cli_parses_sync_with_pair_id() {
        let cli = Cli::parse_from(["adagio", "sync", "pair-abc"]);
        match cli.command {
            Commands::Sync { pair_id: Some(id) } => assert_eq!(id, "pair-abc"),
            other => panic!("unexpected: {:?}", other),
        }
    }

    #[test]
    fn cli_parses_sync_without_pair_id() {
        let cli = Cli::parse_from(["adagio", "sync"]);
        assert!(matches!(cli.command, Commands::Sync { pair_id: None }));
    }

    #[test]
    fn cli_parses_daemon_start() {
        let cli = Cli::parse_from(["adagio", "daemon", "start"]);
        assert!(matches!(
            cli.command,
            Commands::Daemon {
                command: DaemonCommand::Start
            }
        ));
    }
}
