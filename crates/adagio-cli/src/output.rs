use std::io::Write;

use crossterm::style::Color;

// ── Palette ───────────────────────────────────────────────────────────────────

const RESET: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";

fn color(c: Color, s: &str) -> String {
    let code = match c {
        Color::Green => "\x1b[32m",
        Color::DarkGreen => "\x1b[2;32m",
        Color::Yellow => "\x1b[33m",
        Color::DarkYellow => "\x1b[2;33m",
        Color::Red => "\x1b[31m",
        Color::DarkRed => "\x1b[2;31m",
        Color::Cyan => "\x1b[36m",
        Color::DarkCyan => "\x1b[2;36m",
        Color::Magenta => "\x1b[35m",
        Color::DarkMagenta => "\x1b[2;35m",
        Color::DarkGrey => "\x1b[90m",
        Color::White => "\x1b[97m",
        Color::Blue => "\x1b[34m",
        _ => "",
    };
    format!("{code}{s}{RESET}")
}

fn bold(s: &str) -> String {
    format!("{BOLD}{s}{RESET}")
}

fn dim(s: &str) -> String {
    format!("{DIM}{s}{RESET}")
}

// ── Status icons ──────────────────────────────────────────────────────────────

/// Return a colored status icon for the given sync status string.
pub fn status_icon(status: &str) -> String {
    match status {
        "idle" => color(Color::DarkGreen, "✓"),
        "syncing" => color(Color::Cyan, "⟳"),
        "paused" => color(Color::DarkYellow, "⏸"),
        "error" => color(Color::Red, "✗"),
        _ => color(Color::DarkGrey, "·"),
    }
}

/// Return a colored label for the given sync status.
pub fn status_label(status: &str) -> String {
    match status {
        "idle" => color(Color::DarkGreen, "idle"),
        "syncing" => color(Color::Cyan, "syncing"),
        "paused" => color(Color::Yellow, "paused"),
        "error" => color(Color::Red, "error"),
        _ => dim(status),
    }
}

/// Return a colored upload arrow.
pub fn upload_arrow() -> String {
    color(Color::Cyan, "↑")
}

/// Return a colored download arrow.
pub fn download_arrow() -> String {
    color(Color::Magenta, "↓")
}

/// Return a colored error marker.
pub fn error_marker() -> String {
    color(Color::Red, "✗")
}

// ── Divider ───────────────────────────────────────────────────────────────────

pub fn divider(width: usize) -> String {
    dim(&"─".repeat(width))
}

pub fn print_divider(width: usize) {
    eprintln!("{}", divider(width));
}

// ── Header ────────────────────────────────────────────────────────────────────

pub fn print_header(label: &str, detail: &str) {
    let dot = color(Color::DarkGrey, "·");
    let app = bold(&color(Color::White, "adagio"));
    let detail_s = dim(detail);
    eprintln!("  {app}  {dot}  {label}  {dot}  {detail_s}");
}

// ── Table output ──────────────────────────────────────────────────────────────

/// Print a table header row with column names and underlines.
pub fn print_table_header(cols: &[&str], widths: &[usize]) {
    let row: String = cols
        .iter()
        .zip(widths.iter())
        .map(|(col, &w)| format!("{:<w$}  ", dim(col), w = w + 9)) // +9 for dim escape codes
        .collect();
    eprintln!("{row}");
    let underline: String = widths
        .iter()
        .map(|&w| format!("{}  ", dim(&"─".repeat(w))))
        .collect();
    eprintln!("{underline}");
}

/// Print a single table data row.
pub fn print_table_row(cols: &[&str], widths: &[usize]) {
    let row: String = cols
        .iter()
        .zip(widths.iter())
        .map(|(col, &w)| format!("{:<w$}  ", col, w = w))
        .collect();
    println!("{row}");
}

// ── Pair summary ──────────────────────────────────────────────────────────────

/// Print a single pair status line in the compact dashboard style.
pub fn print_pair_line(local_root: &str, status: &str, file_count: usize, last_sync: &str) {
    let icon = status_icon(status);
    let label = status_label(status);
    let root = shorten_path(local_root, 32);
    let files = if file_count > 0 {
        dim(&format!("{file_count:>5} files"))
    } else {
        dim("        ")
    };
    let last = dim(last_sync);
    println!("  {icon}  {root:<32}  {label:<12}  {files}   {last}");
}

/// Print an activity entry in a stream-friendly format.
pub fn print_activity_line(verb: &str, path: &str, who: &str, when: &str) {
    let icon = match verb {
        "uploaded" | "upload" => upload_arrow(),
        "downloaded" | "download" => download_arrow(),
        "deleted" | "delete" => color(Color::Red, "✕"),
        "conflict" => color(Color::Yellow, "⚡"),
        _ => color(Color::DarkGrey, "·"),
    };
    let path_s = color(Color::White, &shorten_path(path, 40));
    let who_s = dim(who);
    let when_s = dim(when);
    println!("  {icon}  {path_s:<46}  {who_s:<12}  {when_s}");
}

// ── JSON output ───────────────────────────────────────────────────────────────

/// Write a `serde_json::Value` as pretty-printed JSON to `stdout`.
pub fn print_json(v: &serde_json::Value) {
    write_json(&mut std::io::stdout(), v).ok();
}

pub fn write_json<W: Write>(w: &mut W, v: &serde_json::Value) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".to_string());
    writeln!(w, "{s}")
}

// ── Plain messages ────────────────────────────────────────────────────────────

pub fn print_ok(msg: &str) {
    println!("  {}  {msg}", color(Color::Green, "✓"));
}

pub fn print_err(msg: &str) {
    eprintln!("  {}  {}", color(Color::Red, "✗"), color(Color::Red, msg));
}

pub fn print_warn(msg: &str) {
    eprintln!("  {}  {msg}", color(Color::Yellow, "⚠"));
}

// ── Uptime ────────────────────────────────────────────────────────────────────

pub fn format_uptime(secs: u64) -> String {
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h {m}m {s}s")
    } else if m > 0 {
        format!("{m}m {s}s")
    } else {
        format!("{s}s")
    }
}

/// Format a byte count human-readably.
pub fn format_bytes(bytes: u64) -> String {
    if bytes >= 1_000_000_000 {
        format!("{:.1} GB", bytes as f64 / 1_000_000_000.0)
    } else if bytes >= 1_000_000 {
        format!("{:.1} MB", bytes as f64 / 1_000_000.0)
    } else if bytes >= 1_000 {
        format!("{:.0} KB", bytes as f64 / 1_000.0)
    } else {
        format!("{bytes} B")
    }
}

/// Shorten a path for display, abbreviating the home dir as `~`.
pub fn shorten_path(path: &str, max: usize) -> String {
    let p = if let Ok(home) = std::env::var("HOME") {
        path.replace(&home, "~")
    } else {
        path.to_string()
    };
    if p.len() <= max {
        p
    } else {
        format!("…{}", &p[p.len().saturating_sub(max - 1)..])
    }
}

// ── Spinner ───────────────────────────────────────────────────────────────────

/// Create and return an indicatif spinner for an in-progress operation.
pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template(&format!("  {{spinner:.cyan}}  {}", dim("{msg}")))
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn spinner_ok(pb: &indicatif::ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", color(Color::Green, "✓")));
}

pub fn spinner_err(pb: &indicatif::ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{} {msg}", color(Color::Red, "✗")));
}

// ── Keyboard hints ────────────────────────────────────────────────────────────

/// Print a row of keyboard shortcut hints.
pub fn print_hints(hints: &[(&str, &str)]) {
    let s: Vec<String> = hints
        .iter()
        .map(|(key, label)| {
            format!(
                "{}{}{}",
                color(Color::DarkGrey, "["),
                bold(key),
                color(Color::DarkGrey, &format!("] {label}")),
            )
        })
        .collect();
    eprintln!("  {}", s.join(dim("  ·  ").as_str()));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_json_produces_valid_json() {
        let v = serde_json::json!({"key": "val", "num": 42});
        let mut buf = Vec::new();
        write_json(&mut buf, &v).unwrap();
        let s = String::from_utf8(buf).unwrap();
        serde_json::from_str::<serde_json::Value>(&s).expect("must be valid JSON");
        assert!(!s.contains('\x1b'), "no ANSI codes: {s}");
    }

    #[test]
    fn format_uptime_seconds_only() {
        assert_eq!(format_uptime(45), "45s");
    }

    #[test]
    fn format_uptime_minutes_and_seconds() {
        assert_eq!(format_uptime(125), "2m 5s");
    }

    #[test]
    fn format_uptime_hours() {
        assert_eq!(format_uptime(7261), "2h 1m 1s");
    }

    #[test]
    fn format_bytes_various() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1500), "2 KB");
        assert_eq!(format_bytes(2_500_000), "2.5 MB");
        assert_eq!(format_bytes(1_200_000_000), "1.2 GB");
    }

    #[test]
    fn shorten_path_short_passthrough() {
        assert_eq!(shorten_path("/tmp/test", 40), "/tmp/test");
    }
}
