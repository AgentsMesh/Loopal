use clap::Args;

#[derive(Args, Debug, Default)]
pub struct ParentOnlyArgs {
    /// Resume session by id (omit value to resume latest)
    #[arg(
        short,
        long,
        num_args = 0..=1,
        default_missing_value = "",
        value_name = "ID",
    )]
    pub resume: Option<String>,

    /// Speak ACP (Agent Client Protocol) over stdio for IDE integration
    #[arg(long)]
    pub acp: bool,

    /// Run as agent server (TCP listener for multi-client IDE/CLI)
    #[arg(long)]
    pub server: bool,

    #[arg(long, hide = true)]
    pub serve: bool,

    /// Run in an isolated git worktree (auto-created under .claude/worktrees/)
    #[arg(long)]
    pub worktree: bool,

    #[arg(long, hide = true)]
    pub test_provider: Option<String>,

    /// Bind address for hub-of-hubs federation (advanced)
    #[arg(long, value_name = "ADDR")]
    pub meta_hub: Option<String>,

    #[arg(long, hide = true, requires = "meta_hub", value_parser = parse_pid)]
    pub meta_hub_parent_pid: Option<u32>,

    /// TCP address of an existing hub to attach this agent to
    #[arg(long, value_name = "ADDR")]
    pub attach_hub: Option<String>,

    /// Bearer token for hub authentication
    #[arg(long, value_name = "TOKEN")]
    pub hub_token: Option<String>,

    #[arg(long, hide = true)]
    pub hub_only: bool,

    /// List active hub processes on this machine
    #[arg(long)]
    pub list_hubs: bool,

    /// Attach to a hub by its process id
    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub attach_hub_pid: Option<u32>,

    /// Kill a hub process by pid
    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub kill_hub: Option<u32>,
}

pub(crate) fn parse_pid(s: &str) -> Result<u32, String> {
    let pid: u32 = s.parse().map_err(|e| format!("invalid pid {s:?}: {e}"))?;
    if pid == 0 {
        return Err("pid must be > 0".into());
    }
    Ok(pid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_pid_accepts_positive() {
        assert_eq!(parse_pid("42").unwrap(), 42);
    }

    #[test]
    fn parse_pid_rejects_zero() {
        assert!(parse_pid("0").is_err());
    }

    #[test]
    fn parse_pid_rejects_negative() {
        assert!(parse_pid("-1").is_err());
    }

    #[test]
    fn parse_pid_rejects_garbage() {
        assert!(parse_pid("abc").is_err());
    }
}
