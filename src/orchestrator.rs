use crate::{
    adapter::{claude::ClaudeAdapter, Adapter, AdapterRequest},
    cli::Cli,
    error::PolycodeError,
};
use std::env;

pub struct Orchestrator;

impl Orchestrator {
    pub async fn run(cli: &Cli) -> Result<(), PolycodeError> {
        let prompt = cli.prompt.as_deref().unwrap_or("").to_string();
        let cwd = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

        // Phase 1: always use ClaudeAdapter. Router plugs in here in Phase 4.
        let adapter_id = cli.tool.as_deref().unwrap_or("claude-code");

        if adapter_id != "claude-code" {
            eprintln!(
                "polycode: adapter '{}' not yet implemented — only 'claude-code' available in this build",
                adapter_id
            );
            return Err(PolycodeError::NoAdapter(format!(
                "adapter '{}' not available",
                adapter_id
            )));
        }

        let adapter = ClaudeAdapter::new();

        if cli.dry_run {
            let model = cli.model.as_deref().unwrap_or("<adapter default>");
            println!("routing decision:");
            println!("  adapter : {}", adapter.id());
            println!("  model   : {}", model);
            println!("  prompt  : {}", prompt);
            println!("[dry-run] not invoking.");
            return Ok(());
        }

        // Health check before invoking.
        let health = adapter.health_check().await;
        if !health.is_ok() {
            let reason = match &health {
                crate::adapter::HealthStatus::Unavailable { reason } => reason.clone(),
                crate::adapter::HealthStatus::Degraded { reason } => reason.clone(),
                _ => unreachable!(),
            };
            eprintln!("polycode: claude-code is unavailable — {}", reason);
            return Err(PolycodeError::NoAdapter(reason));
        }

        let mut req = AdapterRequest::new(prompt, cwd);
        if let Some(model) = &cli.model {
            req = req.with_model(model.clone());
        }

        match adapter.invoke(req).await {
            Ok(result) => {
                print!("{}", result.text);
                Ok(())
            }
            Err(e) => {
                eprintln!("polycode: {}", e);
                Err(PolycodeError::Adapter(e))
            }
        }
    }
}
