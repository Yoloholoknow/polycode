mod adapter;
mod cli;
mod error;
mod orchestrator;
mod quota;

use clap::Parser;
use cli::{Cli, Commands};
use orchestrator::Orchestrator;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::WARN.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();

    // Dispatch subcommands
    if let Some(cmd) = &cli.command {
        match cmd {
            Commands::Setup => {
                eprintln!("polycode setup: coming in Phase 2 — will select tools and auto-install missing ones.");
                std::process::exit(0);
            }
            Commands::Doctor => {
                eprintln!("polycode doctor: coming in Phase 2 — will check adapter health and quota state.");
                std::process::exit(0);
            }
            Commands::Status => {
                eprintln!("polycode status: coming in Phase 2 — will show per-adapter quota usage.");
                std::process::exit(0);
            }
        }
    }

    // Default: route prompt
    if cli.prompt.is_none() && !cli.dry_run {
        eprintln!("polycode: no prompt given. Use --help for usage.");
        std::process::exit(1);
    }

    if let Err(e) = Orchestrator::run(&cli).await {
        tracing::debug!("{:?}", e);
        std::process::exit(1);
    }
}
