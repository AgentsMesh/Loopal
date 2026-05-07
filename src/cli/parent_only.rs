use clap::Args;

#[derive(Args, Debug, Default)]
pub struct ParentOnlyArgs {
    #[arg(short, long, num_args = 0..=1, default_missing_value = "")]
    pub resume: Option<String>,

    #[arg(long)]
    pub acp: bool,

    #[arg(long)]
    pub server: bool,

    #[arg(long, hide = true)]
    pub serve: bool,

    #[arg(long)]
    pub worktree: bool,

    #[arg(long, hide = true)]
    pub test_provider: Option<String>,

    #[arg(long)]
    pub meta_hub: Option<String>,

    #[arg(long)]
    pub attach_hub: Option<String>,

    #[arg(long)]
    pub hub_token: Option<String>,

    #[arg(long, hide = true)]
    pub hub_only: bool,

    #[arg(long)]
    pub list_hubs: bool,

    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub attach_hub_pid: Option<u32>,

    #[arg(long, value_name = "PID", value_parser = parse_pid)]
    pub kill_hub: Option<u32>,
}

fn parse_pid(s: &str) -> Result<u32, String> {
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
