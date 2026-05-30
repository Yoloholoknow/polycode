use clap::{Parser, Subcommand, Args};

#[derive(Parser, Debug)]
#[command(
    name = "polycode",
    version,
    about = "AI coding router — routes prompts to the best tool+model, stretches quota via fallback",
    long_about = None,
    // Allow prompts that look like subcommand names to fall through
    disable_help_subcommand = false,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    // ── Default "run" mode (no subcommand) ───────────────────────────────────

    /// Prompt to send to the AI coding assistant
    pub prompt: Option<String>,

    /// Force a specific adapter (e.g. claude-code, codex, gemini-api, opencode, copilot, coderabbit)
    #[arg(long, short = 't', value_name = "ADAPTER", global = false)]
    pub tool: Option<String>,

    /// Force a specific model (e.g. sonnet, opus, haiku, gpt-4o)
    #[arg(long, short = 'm', value_name = "MODEL", global = false)]
    pub model: Option<String>,

    /// Show routing decision without invoking the tool
    #[arg(long, global = false)]
    pub dry_run: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Interactive onboarding: select tools, auto-install missing ones
    Setup,

    /// Check which adapters are installed and healthy
    Doctor,

    /// Show quota state for all enabled adapters
    Status,

    /// Initialize .polycode/ in the current directory
    Init,

    /// View or manage the project journal
    Journal(JournalArgs),
}

#[derive(Args, Debug)]
pub struct JournalArgs {
    #[command(subcommand)]
    pub action: Option<JournalAction>,
}

#[derive(Subcommand, Debug)]
pub enum JournalAction {
    /// Reset the journal to the initial template
    Clear,

    /// Open the journal in $EDITOR
    Edit,
}
