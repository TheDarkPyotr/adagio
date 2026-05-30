use std::sync::Arc;
use std::time::{Duration, Instant};

use adagio_ipc::{DaemonClient, DaemonRequest};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute, terminal,
};

use crate::output::{bold, cyan, dim, format_uptime, green, shorten_path, status_icon_colored, status_label};

const REFRESH_MS: u64 = 2000;

// Column widths for the pairs table — all VISUAL widths (plain text only).
const COL_FOLDER: usize = 36;
const COL_STATUS: usize = 10;
const COL_LAST: usize = 12;
// Total rule width.
const RULE_W: usize = COL_FOLDER + COL_STATUS + COL_LAST + 8; // 8 = spacing

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Default)]
struct State {
    engine_status: String,
    active_files: u64,
    uptime_secs: u64,
    account: String,
    pairs: Vec<Pair>,
}

#[derive(Default)]
struct Pair {
    local_root: String,
}

async fn fetch(client: &Arc<DaemonClient>) -> State {
    let mut s = State::default();

    if let Ok(v) = client.request(DaemonRequest::GetStatus).await {
        s.engine_status = v["status"].as_str().unwrap_or("idle").to_string();
        s.active_files = v["active_file_count"].as_u64().unwrap_or(0);
    }
    if let Ok(v) = client.request(DaemonRequest::Ping).await {
        s.uptime_secs = v["uptime_secs"].as_u64().unwrap_or(0);
    }
    if let Ok(v) = client.request(DaemonRequest::ListAccounts).await {
        if let Some(a) = v.as_array().and_then(|arr| arr.first()) {
            let name = a["display_name"].as_str().unwrap_or("");
            let url = a["server_url"]
                .as_str()
                .unwrap_or("")
                .trim_start_matches("https://")
                .trim_start_matches("http://");
            s.account = format!("{name} @ {url}");
        }
    }
    if let Ok(v) = client.request(DaemonRequest::ListPairs).await {
        if let Some(arr) = v.as_array() {
            for p in arr {
                s.pairs.push(Pair {
                    local_root: p["local_root"].as_str().unwrap_or("?").to_string(),
                });
            }
        }
    }
    s
}

// ── Rendering ─────────────────────────────────────────────────────────────────
// Rule: pad plain text to the desired column width FIRST, then apply ANSI color.
// Never do format!("{:<N}", colored_string) — ANSI bytes inflate the count.

fn rule() -> String {
    dim(&"─".repeat(RULE_W))
}

fn render(s: &State, flash: Option<&str>) {
    use std::io::Write as _;
    let mut o = String::new();

    // Home + clear.
    o.push_str("\x1b[H\x1b[2J");

    // ── Header ────────────────────────────────────────────────────────────────
    // Plain text fields, spaces between them, no padding needed here.
    let up = if s.uptime_secs > 0 {
        dim(&format!("up {}", format_uptime(s.uptime_secs)))
    } else {
        String::new()
    };
    let dot = dim("·");
    o.push_str(&format!(
        "\n  {}  {dot}  {}  {dot}  {up}\n\n",
        bold("adagio"),
        dim(&s.account),
    ));
    o.push_str(&format!("  {}\n\n", rule()));

    // ── Column headers ────────────────────────────────────────────────────────
    // Pad plain text then dim — this produces correct visual alignment.
    o.push_str(&format!(
        "   {folder_h}  {status_h}  {last_h}\n",
        folder_h = dim(&format!("{:<COL_FOLDER$}", "local folder")),
        status_h = dim(&format!("{:<COL_STATUS$}", "status")),
        last_h = dim("last sync"),
    ));
    o.push_str(&format!(
        "   {folder_u}  {status_u}  {last_u}\n",
        folder_u = dim(&"─".repeat(COL_FOLDER)),
        status_u = dim(&"─".repeat(COL_STATUS)),
        last_u = dim(&"─".repeat(COL_LAST)),
    ));

    // ── Pairs ─────────────────────────────────────────────────────────────────
    if s.pairs.is_empty() {
        o.push_str(&format!(
            "\n  {}\n  {}\n",
            dim("No sync pairs configured."),
            dim("Add a pair via the desktop app or  adagio pairs add"),
        ));
    } else {
        for pair in &s.pairs {
            // Step 1: get plain-text versions padded to exact column widths.
            let root_plain = shorten_path(&pair.local_root, COL_FOLDER);
            let root_padded = format!("{root_plain:<COL_FOLDER$}");

            // status_label already pads internally (see output.rs).
            let label = status_label(&s.engine_status, COL_STATUS);

            // last sync: plain text padded, then dimmed.
            let last = dim(&format!("{:<COL_LAST$}", "—"));

            // Step 2: color the icon (1 visual char, no padding involved).
            let icon = status_icon_colored(&s.engine_status);

            o.push_str(&format!("  {icon} {root_padded}  {label}  {last}\n"));
        }
    }

    // Active transfer count while syncing.
    if s.engine_status == "syncing" && s.active_files > 0 {
        o.push_str(&format!(
            "\n  {}  {} file(s) active\n",
            cyan("⟳"),
            s.active_files
        ));
    }

    o.push_str(&format!("\n  {}\n", rule()));

    // ── Flash or hints ────────────────────────────────────────────────────────
    if let Some(msg) = flash {
        o.push_str(&format!("\n  {}\n\n", green(msg)));
    } else {
        o.push_str(&format!(
            "\n  {}  {}  {}  {}  {}\n\n",
            hint("s", "sync"),
            hint("p", "pause"),
            hint("r", "resume"),
            hint("a", "activity"),
            hint("q", "quit"),
        ));
    }

    print!("{o}");
    std::io::stdout().flush().ok();
}

/// Keyboard hint: `[key] label` — bold key, dim brackets and label.
fn hint(key: &str, label: &str) -> String {
    format!("{}{}{}", dim("["), bold(key), dim(&format!("] {label}")))
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Drop guard that restores terminal state even on panic.
struct TermRestoreGuard;
impl Drop for TermRestoreGuard {
    fn drop(&mut self) {
        let _ = execute!(std::io::stdout(), cursor::Show, cursor::MoveToNextLine(1));
        let _ = terminal::disable_raw_mode();
    }
}

/// Run the interactive live dashboard.
///
/// Falls back to a single status snapshot when stdout is not a TTY (pipe/redirect).
pub async fn run_dashboard(client: Arc<DaemonClient>) {
    if !crossterm::tty::IsTty::is_tty(&std::io::stdout()) {
        let _ = crate::handlers::status::run_status(&client, false).await;
        return;
    }

    terminal::enable_raw_mode().ok();
    execute!(std::io::stdout(), cursor::Hide).ok();

    // Ensure terminal is always restored even on panic.
    let _guard = TermRestoreGuard;

    let mut state = fetch(&client).await;
    let mut refresh = Instant::now()
        .checked_sub(Duration::from_secs(10))
        .unwrap_or_else(Instant::now);
    let mut flash: Option<(String, Instant)> = None;

    loop {
        // Refresh data on first paint and every REFRESH_MS thereafter.
        if refresh.elapsed() >= Duration::from_millis(REFRESH_MS) {
            state = fetch(&client).await;
            refresh = Instant::now();
        }

        let msg = flash
            .as_ref()
            .filter(|(_, t)| t.elapsed() < Duration::from_secs(3))
            .map(|(m, _)| m.as_str());
        render(&state, msg);

        if flash
            .as_ref()
            .map_or(false, |(_, t)| t.elapsed() >= Duration::from_secs(3))
        {
            flash = None;
        }

        // Poll for keypress with short timeout so the refresh loop can fire.
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(Event::Key(key)) = event::read() {
                match (key.code, key.modifiers) {
                    (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => break,

                    (KeyCode::Char('s'), _) => {
                        let _ = client
                            .request(DaemonRequest::TriggerSync {
                                pair_id: String::new(),
                            })
                            .await;
                        state = fetch(&client).await;
                        refresh = Instant::now();
                        flash = Some(("Sync triggered".to_string(), Instant::now()));
                    }
                    (KeyCode::Char('p'), _) => {
                        let _ = client.request(DaemonRequest::PauseSyncAll).await;
                        state = fetch(&client).await;
                        refresh = Instant::now();
                        flash = Some(("Sync paused".to_string(), Instant::now()));
                    }
                    (KeyCode::Char('r'), _) => {
                        let _ = client.request(DaemonRequest::ResumeSyncAll).await;
                        state = fetch(&client).await;
                        refresh = Instant::now();
                        flash = Some(("Sync resumed".to_string(), Instant::now()));
                    }
                    (KeyCode::Char('a'), _) => {
                        // Temporarily restore normal terminal to print activity.
                        terminal::disable_raw_mode().ok();
                        execute!(std::io::stdout(), cursor::Show, cursor::MoveToNextLine(1)).ok();
                        let _ =
                            crate::handlers::activity::run_activity(&client, 20, None, false).await;
                        println!("\n  {}", dim("Press any key to return…"));
                        terminal::enable_raw_mode().ok();
                        execute!(std::io::stdout(), cursor::Hide).ok();
                        event::read().ok();
                        // Force immediate redraw.
                        refresh = Instant::now()
                            .checked_sub(Duration::from_secs(10))
                            .unwrap_or_else(Instant::now);
                    }
                    _ => {}
                }
            }
        }
    }

    // _guard drops here and restores terminal.
}
