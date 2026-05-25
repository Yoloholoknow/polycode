use clap::Parser;

#[derive(Parser, Debug)]
#[command(
    name = "polycode",
    version,
    about = "AI coding router — routes prompts to the best tool+model, stretches quota via fallback",
    long_about = None,
)]
pub struct Cli {
    /// The prompt to send to the AI coding assistant
    pub prompt: Option<String>,

    /// Force a specific adapter (e.g. claude-code, codex, gemini-cli, opencode, copilot)
    #[arg(long, short = 't', value_name = "ADAPTER")]
    pub tool: Option<String>,

    /// Force a specific model (e.g. sonnet, opus, haiku, gpt-4o)
    #[arg(long, short = 'm', value_name = "MODEL")]
    pub model: Option<String>,

    /// Show the routing decision without actually invoking the tool
    #[arg(long)]
    pub dry_run: bool,
}
