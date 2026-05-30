use std::io::Write;

// ── JSON output ───────────────────────────────────────────────────────────────

/// Write a `serde_json::Value` as pretty-printed JSON to `stdout`.
///
/// Only valid JSON is written — no ANSI codes, headers, or trailing decoration.
pub fn print_json(v: &serde_json::Value) {
    write_json(&mut std::io::stdout(), v).ok();
}

/// Write a `serde_json::Value` as pretty-printed JSON to any writer.
///
/// Used by `print_json` and in unit tests (where we capture output).
pub fn write_json<W: Write>(w: &mut W, v: &serde_json::Value) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(v).unwrap_or_else(|_| "{}".to_string());
    writeln!(w, "{s}")
}

// ── Human-readable table output ───────────────────────────────────────────────

/// Print a table header row with column names and underlines.
///
/// Each element of `cols` is a column name; `widths` is its minimum display width.
pub fn print_table_header(cols: &[&str], widths: &[usize]) {
    let row: String = cols
        .iter()
        .zip(widths.iter())
        .map(|(col, &w)| format!("{:<w$}  ", col, w = w))
        .collect();
    eprintln!("{row}");
    let underline: String = widths
        .iter()
        .map(|&w| format!("{:-<w$}  ", "", w = w))
        .collect();
    eprintln!("{underline}");
}

/// Print a single table data row.
///
/// `cols` are the cell values; `widths` are the minimum column widths.
pub fn print_table_row(cols: &[&str], widths: &[usize]) {
    let row: String = cols
        .iter()
        .zip(widths.iter())
        .map(|(col, &w)| format!("{:<w$}  ", col, w = w))
        .collect();
    println!("{row}");
}

/// Print a plain message (no table formatting).
pub fn print_ok(msg: &str) {
    println!("{msg}");
}

/// Print a plain message to stderr.
pub fn print_err(msg: &str) {
    eprintln!("error: {msg}");
}

/// Format uptime seconds as a human-readable string, e.g. "2h 4m 1s".
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

#[cfg(test)]
mod tests {
    use super::*;

    // T008 — write_json produces only parseable JSON, no ANSI codes.
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
}
