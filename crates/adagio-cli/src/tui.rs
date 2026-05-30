use std::sync::Arc;
use std::time::{Duration, Instant};

use adagio_ipc::{DaemonClient, DaemonRequest};
use crossterm::{
    cursor,
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{self},
};

use crate::output::{format_uptime, shorten_path};

const REFRESH_MS: u64 = 2000;
const W: usize = 70;

// ── Color helpers ─────────────────────────────────────────────────────────────

fn green(s: &str) -> String {
    format!("\x1b[32m{s}\x1b[0m")
}
fn cyan(s: &str) -> String {
    format!("\x1b[36m{s}\x1b[0m")
}
fn yellow(s: &str) -> String {
    format!("\x1b[33m{s}\x1b[0m")
}
fn red(s: &str) -> String {
    format!("\x1b[31m{s}\x1b[0m")
}
fn dim(s: &str) -> String {
    format!("\x1b[2m{s}\x1b[0m")
}
fn bold(s: &str) -> String {
    format!("\x1b[1m{s}\x1b[0m")
}
fn rule(n: usize) -> String {
    dim(&"─".repeat(n))
}

fn status_icon(s: &str) -> String {
    match s {
        "syncing" => cyan("⟳"),
        "paused" => yellow("⏸"),
        "error" => red("✗"),
        _ => green("✓"),
    }
}

fn status_label(s: &str) -> String {
    match s {
        "syncing" => cyan("syncing"),
        "paused" => yellow("paused"),
        "error" => red("error"),
        _ => dim("idle"),
    }
}

fn relative_time(unix_ms: i64) -> String {
    let now = chrono::Utc::now().timestamp_millis();
    let secs = ((now - unix_ms) / 1000).max(0);
    let s = if secs < 60 {
        "just now".to_string()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86400 {
        format!("{} hr ago", secs / 3600)
    } else {
        format!("{} d ago", secs / 86400)
    };
    dim(&s)
}

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
    remote_root: String,
    last_sync_ms: Option<i64>,
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
        if let Some(a) = v.as_array().and_then(|a| a.first()) {
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
                    remote_root: p["remote_root"].as_str().unwrap_or("/").to_string(),
                    last_sync_ms: None,
                });
            }
        }
    }
    s
}

// ── Rendering ─────────────────────────────────────────────────────────────────

fn render(s: &State, flash: Option<&str>) {
    use std::io::Write as _;
    let mut o = String::new();

    // Clear screen and move to origin.
    o.push_str("\x1b[H\x1b[2J");

    // ── Header ────────────────────────────────────────────────────────────────
    let up = if s.uptime_secs > 0 {
        dim(&format!("up {}", format_uptime(s.uptime_secs)))
    } else {
        String::new()
    };
    let dot = dim("·");
    o.push_str(&format!(
        "\n  {}  {dot}  {}  {dot}  {up}\n\n",
        bold("adagio"),
        dim(&s.account)
    ));
    o.push_str(&format!("  {}\n\n", rule(W)));

    // ── Pairs ─────────────────────────────────────────────────────────────────
    o.push_str(&format!(
        "  {} {:<34} {:<14} {}\n",
        dim(" "),
        dim("folder"),
        dim("status"),
        dim("last sync")
    ));
    o.push_str(&format!(
        "  {} {}  {}  {}\n",
        dim(" "),
        dim(&"─".repeat(34)),
        dim(&"─".repeat(14)),
        dim(&"─".repeat(12))
    ));

    if s.pairs.is_empty() {
        o.push_str(&format!(
            "\n  {}\n  {}\n",
            dim("No sync pairs configured."),
            dim("Add one via the desktop app or  adagio pairs add  .")
        ));
    } else {
        for pair in &s.pairs {
            let icon = status_icon(&s.engine_status);
            let root = shorten_path(&pair.local_root, 34);
            let label = status_label(&s.engine_status);
            let last = pair
                .last_sync_ms
                .map(relative_time)
                .unwrap_or_else(|| dim("never"));
            o.push_str(&format!("  {icon} {root:<34}  {label:<14}  {last}\n"));
        }
    }

    // Active file count during sync.
    if s.engine_status == "syncing" && s.active_files > 0 {
        o.push_str(&format!("\n  {}  {} active\n", cyan("⟳"), s.active_files));
    }

    o.push_str(&format!("\n  {}\n", rule(W)));

    // ── Flash message or hints ────────────────────────────────────────────────
    if let Some(msg) = flash {
        o.push_str(&format!("\n  {}\n\n", green(msg)));
    } else {
        o.push_str(&format!(
            "\n  {}   {}   {}   {}   {}\n\n",
            hint("s", "sync"),
            hint("p", "pause"),
            hint("r", "resume"),
            hint("a", "activity"),
            hint("q", "quit")
        ));
    }

    print!("{o}");
    std::io::stdout().flush().ok();
}

fn hint(k: &str, label: &str) -> String {
    format!("{}{}{}", dim("["), bold(k), dim(&format!("] {label}")))
}

// ── Entry point ───────────────────────────────────────────────────────────────

/// Run the interactive dashboard. Falls back to a one-shot status print if
/// stdout is not a TTY (e.g. when piped).
pub async fn run_dashboard(client: Arc<DaemonClient>) {
    if !crossterm::tty::IsTty::is_tty(&std::io::stdout()) {
        let _ = crate::handlers::status::run_status(&client, false).await;
        return;
    }

    terminal::enable_raw_mode().ok();
    execute!(std::io::stdout(), cursor::Hide).ok();

    let mut state = fetch(&client).await;
    let mut refresh = Instant::now() - Duration::from_secs(10);
    let mut flash: Option<(String, Instant)> = None;

    loop {
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
                        flash = Some(("Sync triggered for all pairs".to_string(), Instant::now()));
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
                        // Drop raw mode briefly to show activity log.
                        terminal::disable_raw_mode().ok();
                        execute!(std::io::stdout(), cursor::Show, cursor::MoveToNextLine(1)).ok();
                        let _ =
                            crate::handlers::activity::run_activity(&client, 20, None, false).await;
                        println!("\n  Press any key to return…");
                        terminal::enable_raw_mode().ok();
                        execute!(std::io::stdout(), cursor::Hide).ok();
                        event::read().ok(); // wait for keypress
                        refresh = Instant::now() - Duration::from_secs(10); // force redraw
                    }
                    _ => {}
                }
            }
        }
    }

    execute!(std::io::stdout(), cursor::Show, cursor::MoveToNextLine(1)).ok();
    terminal::disable_raw_mode().ok();
}
