/// Errors that the CLI maps to specific exit codes.
#[derive(Debug)]
pub enum CliError {
    /// The daemon returned an application-level error. Exit code 2.
    DaemonError(String),
    /// The daemon is not reachable and could not be started. Exit code 3.
    Unreachable(String),
}

impl CliError {
    /// The exit code to use when this error terminates the process.
    ///
    /// - `DaemonError` → 2
    /// - `Unreachable` → 3
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::DaemonError(_) => 2,
            CliError::Unreachable(_) => 3,
        }
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::DaemonError(msg) => write!(f, "Daemon error: {msg}"),
            CliError::Unreachable(msg) => write!(f, "Daemon not reachable: {msg}"),
        }
    }
}

impl From<anyhow::Error> for CliError {
    fn from(e: anyhow::Error) -> Self {
        let msg = e.to_string();
        if msg.contains("not reachable") || msg.contains("not start") || msg.contains("connect") {
            CliError::Unreachable(msg)
        } else {
            CliError::DaemonError(msg)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T007-a — DaemonError exit code is 2.
    #[test]
    fn exit_code_daemon_err_is_2() {
        assert_eq!(CliError::DaemonError("x".into()).exit_code(), 2);
    }

    // T007-b — Unreachable exit code is 3.
    #[test]
    fn exit_code_unreachable_is_3() {
        assert_eq!(CliError::Unreachable("x".into()).exit_code(), 3);
    }
}
