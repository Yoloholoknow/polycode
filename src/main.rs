mod adapter;
mod cli;
mod error;
mod orchestrator;

use clap::Parser;
use cli::Cli;
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

    if cli.prompt.is_none() && !cli.dry_run {
        eprintln!("polycode: no prompt given. Use --help for usage.");
        std::process::exit(1);
    }

    if let Err(e) = Orchestrator::run(&cli).await {
        tracing::debug!("{:?}", e);
        std::process::exit(1);
    }
}
