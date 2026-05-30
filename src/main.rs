mod adapter;
mod cli;
mod commands;
mod error;
mod journal;
mod orchestrator;
mod quota;

use clap::Parser;
use cli::{Cli, Commands};
use orchestrator::Orchestrator;
use cli::JournalAction;

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
                eprintln!("polycode setup: interactive auto-install coming in a future release. Run `polycode doctor` to check adapter health.");
                std::process::exit(0);
            }
            Commands::Doctor => {
                commands::doctor().await;
                std::process::exit(0);
            }
            Commands::Status => {
                commands::status();
                std::process::exit(0);
            }
            Commands::Init => {
                commands::init();
                std::process::exit(0);
            }
            Commands::Journal(args) => {
                let action = args.action.as_ref().map(|a| match a {
                    JournalAction::Clear => commands::JournalCmd::Clear,
                    JournalAction::Edit  => commands::JournalCmd::Edit,
                });
                std::process::exit(commands::journal(action));
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
