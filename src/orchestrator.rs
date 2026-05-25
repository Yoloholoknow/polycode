use crate::{
    adapter::{self, AdapterRequest, ErrorKind},
    cli::Cli,
    error::PolycodeError,
    quota::{InvocationRecord, QuotaTracker},
};
use std::env;
use std::time::Duration;

const DEFAULT_COOLDOWN: Duration = Duration::from_secs(3600);

pub struct Orchestrator;

impl Orchestrator {
    pub async fn run(cli: &Cli) -> Result<(), PolycodeError> {
        let prompt = cli.prompt.as_deref().unwrap_or("").to_string();
        let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        // Build candidate list.
        // --tool X: forced, strict (no fallback on failure — user asked for that tool).
        // Default: walk DEFAULT_CHAIN with quota fallback.
        let forced = cli.tool.is_some();
        let candidates: Vec<&str> = if let Some(tool) = cli.tool.as_deref() {
            vec![tool]
        } else {
            adapter::DEFAULT_CHAIN.to_vec()
        };

        // Dry-run: resolve first available (not cooling down + binary present) and print.
        if cli.dry_run {
            return Self::dry_run(&candidates, cli, &prompt);
        }

        // Open quota tracker. If it fails, log and continue without tracking.
        let tracker = match QuotaTracker::open() {
            Ok(t) => Some(t),
            Err(e) => {
                tracing::warn!("quota tracker unavailable: {}", e);
                None
            }
        };

        let mut last_error: Option<String> = None;
        let mut tried: Vec<String> = Vec::new();

        for id in &candidates {
            // Skip adapters in quota cooldown (unless forced — forced must surface the error).
            if !forced {
                if let Some(ref t) = tracker {
                    if let Ok(Some(until)) = t.is_cooling_down(id) {
                        let secs_left = (until - now_secs()).max(0);
                        tracing::debug!("{} cooling down for {}s, skipping", id, secs_left);
                        eprintln!("polycode: {} quota cooling down ({}s left), skipping", id, secs_left);
                        tried.push(format!("{} (cooling down)", id));
                        continue;
                    }
                }
            }

            let adapter = match adapter::by_id(id) {
                Some(a) => a,
                None => {
                    eprintln!("polycode: adapter '{}' not recognised", id);
                    last_error = Some(format!("adapter '{}' not recognised", id));
                    tried.push(format!("{} (unknown)", id));
                    if forced {
                        break;
                    }
                    continue;
                }
            };

            // Health check before invoking.
            let health = adapter.health_check().await;
            if !health.is_ok() {
                let reason = match &health {
                    crate::adapter::HealthStatus::Unavailable { reason } => reason.clone(),
                    crate::adapter::HealthStatus::Degraded { reason } => reason.clone(),
                    _ => unreachable!(),
                };
                tracing::debug!("{} unavailable: {}", id, reason);
                if forced {
                    eprintln!("polycode: {} unavailable — {}", id, reason);
                } else {
                    eprintln!("polycode: {} unavailable, skipping ({})", id, reason);
                }
                last_error = Some(format!("{}: {}", id, reason));
                tried.push(format!("{} (unavailable)", id));
                if forced {
                    break;
                }
                continue;
            }

            // Build request.
            let mut req = AdapterRequest::new(&prompt, &cwd);
            if let Some(model) = &cli.model {
                req = req.with_model(model.clone());
            }

            match adapter.invoke(req).await {
                Ok(result) => {
                    // Record success.
                    if let Some(ref t) = tracker {
                        let _ = t.record_invocation(&InvocationRecord {
                            adapter_id: id.to_string(),
                            model: result.model_used.clone().or_else(|| cli.model.clone()),
                            success: true,
                            error_kind: None,
                            input_tokens: result.usage.as_ref().map(|u| u.input),
                            output_tokens: result.usage.as_ref().map(|u| u.output),
                        });
                        let _ = t.clear_cooldown(id);
                    }
                    print!("{}", result.text);
                    return Ok(());
                }
                Err(ErrorKind::QuotaExceeded { reset_hint }) => {
                    let cooldown = reset_hint.unwrap_or(DEFAULT_COOLDOWN);
                    eprintln!(
                        "polycode: {} quota exceeded (cooldown {}s)",
                        id,
                        cooldown.as_secs()
                    );
                    if let Some(ref t) = tracker {
                        let _ = t.record_invocation(&InvocationRecord {
                            adapter_id: id.to_string(),
                            model: cli.model.clone(),
                            success: false,
                            error_kind: Some("QuotaExceeded".to_string()),
                            input_tokens: None,
                            output_tokens: None,
                        });
                        let _ = t.mark_quota_exceeded(id, cooldown);
                    }
                    last_error = Some(format!("{}: quota exceeded", id));
                    tried.push(format!("{} (quota exceeded)", id));
                    if forced {
                        break; // Forced: surface the error, don't silently switch.
                    }
                    continue;
                }
                Err(e) => {
                    eprintln!("polycode: {} error — {}", id, e);
                    if let Some(ref t) = tracker {
                        let _ = t.record_invocation(&InvocationRecord {
                            adapter_id: id.to_string(),
                            model: cli.model.clone(),
                            success: false,
                            error_kind: Some(e.to_string()),
                            input_tokens: None,
                            output_tokens: None,
                        });
                    }
                    last_error = Some(format!("{}: {}", id, e));
                    tried.push(format!("{} (error)", id));
                    if forced {
                        break;
                    }
                    continue;
                }
            }
        }

        let summary = last_error.unwrap_or_else(|| "all adapters unavailable".to_string());
        if tried.len() > 1 {
            eprintln!("polycode: all adapters exhausted — tried: {}", tried.join(", "));
        }
        Err(PolycodeError::NoAdapter(summary))
    }

    fn dry_run(candidates: &[&str], cli: &Cli, prompt: &str) -> Result<(), PolycodeError> {
        let model = cli.model.as_deref().unwrap_or("<adapter default>");
        println!("routing decision:");
        println!("  chain   : {}", candidates.join(" -> "));
        println!("  model   : {}", model);
        println!("  prompt  : {}", prompt);
        println!("[dry-run] not invoking.");
        Ok(())
    }
}

fn now_secs() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
