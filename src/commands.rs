use crate::adapter::{self, HealthStatus};
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
