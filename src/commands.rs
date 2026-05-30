use crate::adapter::{self, HealthStatus};
use crate::journal::Journal;
use crate::quota::QuotaTracker;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── doctor ────────────────────────────────────────────────────────────────────

static INSTALL_HINTS: &[(&str, &str)] = &[
    ("claude-code", "npm install -g @anthropic-ai/claude-code  OR  brew install claude"),
    ("codex",       "npm install -g @openai/codex"),
    ("copilot",     "brew install copilot-cli  OR  gh extension install github/gh-copilot"),
    ("opencode",    "npm install -g opencode  OR  brew install opencode"),
];

fn install_hint(id: &str) -> &'static str {
    INSTALL_HINTS
        .iter()
        .find(|(k, _)| *k == id)
        .map(|(_, v)| *v)
        .unwrap_or("see the tool's documentation")
}

pub async fn doctor() {
    let adapters = adapter::build_all();
    println!("polycode doctor — checking {} adapters\n", adapters.len());

    for a in adapters {
        let id = a.id();
        let status = a.health_check().await;
        match &status {
            HealthStatus::Ok { version } => {
                // Normalize multi-line version strings (e.g. copilot appends an update notice).
                let v = version.lines().next().unwrap_or(version).trim();
                println!("  [ok]          {} — {}", id, v);
            }
            HealthStatus::Degraded { reason } => {
                println!("  [degraded]    {} — {}", id, reason);
            }
            HealthStatus::Unavailable { reason } => {
                println!("  [unavailable] {} — {}", id, reason);
                println!("                  install: {}", install_hint(id));
            }
        }
    }

    println!();
}

// ── status ────────────────────────────────────────────────────────────────────

pub fn status() {
    let tracker = match QuotaTracker::open() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("polycode status: cannot open quota tracker — {}", e);
            return;
        }
    };

    let rows = match tracker.status_rows() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("polycode status: database error — {}", e);
            return;
        }
    };

    if rows.is_empty() {
        println!("polycode status — no invocations recorded yet");
        return;
    }

    println!("polycode status\n");
    println!(
        "  {:<14}  {:<12}  {:<8}  {:<8}  out-tok",
        "adapter", "quota", "calls", "in-tok"
    );
    println!("  {}", "-".repeat(62));

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs() as i64;

    for row in &rows {
        let quota_str = if row.cooldown_until > now {
            let secs_left = row.cooldown_until - now;
            format!("cooldown {}s", secs_left)
        } else {
            "ok".to_string()
        };

        println!(
            "  {:<14}  {:<12}  {:<8}  {:<8}  {}",
            row.adapter_id,
            quota_str,
            row.invocation_count,
            row.total_input_tokens,
            row.total_output_tokens,
        );
    }

    println!();
}

// ── init ──────────────────────────────────────────────────────────────────────

pub fn init() {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(e) => {
            eprintln!("polycode init: cannot determine current directory — {}", e);
            return;
        }
    };

    match Journal::init(&cwd) {
        Ok(path) => println!("polycode init: journal ready at {}", path.display()),
        Err(e)   => eprintln!("polycode init: failed — {}", e),
    }
}

// ── journal ───────────────────────────────────────────────────────────────────

/// Actions dispatched from `polycode journal [subcommand]`.
pub enum JournalCmd {
    Clear,
    Edit,
}

pub fn journal(action: Option<JournalCmd>) -> i32 {
    match action {
        None                    => journal_view(),
        Some(JournalCmd::Clear) => journal_clear(),
        Some(JournalCmd::Edit)  => journal_edit(),
    }
}

fn journal_view() -> i32 {
    match Journal::open() {
        None => {
            eprintln!("polycode journal: no journal found — run `polycode init` in your project root.");
            1
        }
        Some(j) => match j.read() {
            Ok(contents) => { print!("{}", contents); 0 }
            Err(e)       => { eprintln!("polycode journal: read error — {}", e); 1 }
        },
    }
}

fn journal_clear() -> i32 {
    match Journal::open() {
        None => {
            eprintln!("polycode journal: no journal found — run `polycode init` first.");
            1
        }
        Some(j) => match j.clear() {
            Ok(()) => { println!("polycode journal: cleared."); 0 }
            Err(e) => { eprintln!("polycode journal: clear failed — {}", e); 1 }
        },
    }
}

fn journal_edit() -> i32 {
    let j = match Journal::open() {
        Some(j) => j,
        None => {
            eprintln!("polycode journal: no journal found — run `polycode init` first.");
            return 1;
        }
    };

    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| "vi".to_string());

    match std::process::Command::new(&editor).arg(j.path()).status() {
        Ok(s) => {
            if s.success() { 0 } else { s.code().unwrap_or(1) }
        }
        Err(e) => {
            eprintln!("polycode journal: cannot launch editor '{}' — {}", editor, e);
            1
        }
    }
}
