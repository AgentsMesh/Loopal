use clap::Parser;

use loopal_config::load_config;

use crate::cli::Cli;

mod multiprocess;
mod singleprocess;

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    loopal_config::housekeeping::startup_cleanup();
    if let Some(repo_root) = loopal_git::repo_root(&cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }

    let mut config = load_config(&cwd)?;
    cli.apply_overrides(&mut config.settings);

    if cli.acp {
        return loopal_acp::run_acp(config, cwd).await;
    }

    if cli.serve {
        let test_provider = cli
            .test_provider
            .clone()
            .or_else(|| std::env::var("LOOPAL_TEST_PROVIDER").ok());
        if let Some(path) = test_provider {
            return loopal_agent_server::run_agent_server_with_mock(&path).await;
        }
        return loopal_agent_server::run_agent_server().await;
    }

    if cli.no_ipc || cli.resume.is_some() {
        // --resume requires single-process (session history loaded locally)
        return singleprocess::run(cli, cwd, config).await;
    }

    multiprocess::run(&cli, &cwd, &config).await
}

/// Replace the home directory prefix with `~` for compact display.
fn abbreviate_home(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = path.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}
