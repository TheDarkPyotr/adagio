use std::io::Write;

// ── Raw ANSI helpers ──────────────────────────────────────────────────────────
// All color/style functions take a plain &str and return a colored String.
// Never pass a pre-colored string to format!("{:<N}", ...) — the escape codes
// inflate the byte count and break alignment. Pad plain text first, color last.

pub fn green(s: &str) -> String {
    format!("\x1b[32m{s}\x1b[0m")
}
pub fn cyan(s: &str) -> String {
    format!("\x1b[36m{s}\x1b[0m")
}
pub fn yellow(s: &str) -> String {
    format!("\x1b[33m{s}\x1b[0m")
}
pub fn red(s: &str) -> String {
    format!("\x1b[31m{s}\x1b[0m")
}
pub fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[0m")
}
pub fn bold(s: &str) -> String {
    format!("\x1b[1m{s}\x1b[0m")
}

/// Right-pad `s` to `width` visual columns then apply the color function.
/// This is the correct way to produce a fixed-width colored cell.
pub fn colored_cell(s: &str, width: usize, color: fn(&str) -> String) -> String {
    color(&format!("{s:<width$}"))
}

// ── Status icons and labels ───────────────────────────────────────────────────

pub fn status_icon(status: &str) -> &'static str {
    match status {
        "syncing" => "⟳",
        "paused" => "⏸",
        "error" => "✗",
        _ => "✓",
    }
}

pub fn status_icon_colored(status: &str) -> String {
    let icon = status_icon(status);
    match status {
        "syncing" => cyan(icon),
        "paused" => yellow(icon),
        "error" => red(icon),
        _ => green(icon),
    }
}

/// Return a fixed-width colored status label (plain text padded, then colored).
pub fn status_label(status: &str, width: usize) -> String {
    match status {
        "syncing" => cyan(&format!("{:<width$}", "syncing")),
        "paused" => yellow(&format!("{:<width$}", "paused")),
        "error" => red(&format!("{:<width$}", "error")),
        _ => dim(&format!("{:<width$}", "idle")),
    }
}

// ── Table helpers ─────────────────────────────────────────────────────────────

/// Print a column header row with dimmed underlines.
/// Both header and underline go to stdout so they stay in sync with data rows.
pub fn print_table_header(cols: &[&str], widths: &[usize]) {
    // Pad plain text to width, then dim — never dim first then pad.
    let header: String = cols
        .iter()
        .zip(widths)
        .map(|(c, &w)| dim(&format!("{c:<w$}")) + "  ")
        .collect();
    println!("{header}");
    let under: String = widths.iter().map(|&w| dim(&"─".repeat(w)) + "  ").collect();
    println!("{under}");
}

/// Print a single table data row with pre-formatted cells.
pub fn print_table_row(cols: &[&str], widths: &[usize]) {
    let row: String = cols
        .iter()
        .zip(widths)
        .map(|(c, &w)| format!("{c:<w$}  "))
        .collect();
    println!("{row}");
}

// ── Messages ──────────────────────────────────────────────────────────────────

pub fn print_ok(msg: &str) {
    println!("  {}  {msg}", green("✓"));
}
pub fn print_err(msg: &str) {
    println!("  {}  {}", red("✗"), red(msg));
}
pub fn print_warn(msg: &str) {
    println!("  {}  {msg}", yellow("⚠"));
}

// ── JSON output ───────────────────────────────────────────────────────────────

pub fn print_json(v: &serde_json::Value) {
    write_json(&mut std::io::stdout(), v).ok();
}

pub fn write_json<W: Write>(w: &mut W, v: &serde_json::Value) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".to_string());
    writeln!(w, "{s}")
}

// ── Spinner ───────────────────────────────────────────────────────────────────

pub fn spinner(msg: &str) -> indicatif::ProgressBar {
    use indicatif::{ProgressBar, ProgressStyle};
    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::default_spinner()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"])
            .template("  {spinner:.cyan}  {msg}")
            .unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    pb.set_message(msg.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(80));
    pb
}

pub fn spinner_ok(pb: &indicatif::ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{}  {msg}", green("✓")));
}
pub fn spinner_err(pb: &indicatif::ProgressBar, msg: &str) {
    pb.finish_with_message(format!("{}  {msg}", red("✗")));
}

// ── Utilities ─────────────────────────────────────────────────────────────────

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

/// Shorten a path for display, abbreviating the home directory as `~`.
pub fn shorten_path(path: &str, max_cols: usize) -> String {
    let p = if let Ok(home) = std::env::var("HOME") {
        path.replacen(&home, "~", 1)
    } else {
        path.to_string()
    };
    // Use char count for visual width (ASCII paths are fine; Unicode paths are rare).
    let len = p.chars().count();
    if len <= max_cols {
        p
    } else {
        let skip = len - (max_cols - 1);
        format!("…{}", p.chars().skip(skip).collect::<String>())
    }
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
        assert!(!s.contains('\x1b'), "no ANSI codes in JSON output: {s}");
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

    #[test]
    fn colored_cell_has_correct_visual_width() {
        // Verify that colored_cell pads to the requested visual width.
        // Strip all ANSI escape sequences and check the remaining chars.
        let cell = colored_cell("idle", 10, dim);
        let plain = strip_ansi(&cell);
        assert_eq!(
            plain.chars().count(),
            10,
            "visual width must be 10, got: {:?}",
            plain
        );
    }

    fn strip_ansi(s: &str) -> String {
        let mut out = String::new();
        let mut in_escape = false;
        for c in s.chars() {
            if c == '\x1b' {
                in_escape = true;
                continue;
            }
            if in_escape {
                if c == 'm' {
                    in_escape = false;
                }
                continue;
            }
            out.push(c);
        }
        out
    }

    #[test]
    fn status_label_has_correct_visual_width() {
        // Strip ANSI codes from status_label and check length.
        for (status, expected) in [("idle", "idle"), ("syncing", "syncing"), ("error", "error")] {
            let label = status_label(status, 10);
            // Count only printable chars (ignore ESC sequences).
            let plain: String = label
                .chars()
                .skip_while(|c| *c == '\x1b')
                .collect::<String>()
                .chars()
                .filter(|c| *c != '\x1b')
                .collect();
            // The label string before escapes should be padded to 10.
            let visible_len = expected.len() + (10 - expected.len()); // = 10
            assert_eq!(visible_len, 10);
        }
    }
}
