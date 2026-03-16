use clap::Parser;

#[derive(Parser)]
#[command(name = "loopagent", about = "AI coding agent")]
pub struct Cli {
    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Resume a previous session
    #[arg(short, long)]
    pub resume: Option<String>,

    /// Permission mode
    #[arg(short = 'P', long)]
    pub permission: Option<String>,

    /// Start in plan mode
    #[arg(long)]
    pub plan: bool,

    /// Initial prompt (non-interactive)
    pub prompt: Vec<String>,
}
