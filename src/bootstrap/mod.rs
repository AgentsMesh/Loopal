use std::path::PathBuf;

use clap::Parser;

use loopal_config::load_config;

use crate::cli::Cli;

mod hub;
mod multiprocess;

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

    if cli.hub {
        return hub::run_hub(&cli, &cwd, &config).await;
    }

    // Worktree isolation: create worktree before agent starts, clean up after.
    let worktree = if cli.worktree {
        Some(create_session_worktree(&cwd)?)
    } else {
        None
    };
    let effective_cwd = worktree
        .as_ref()
        .map(|wt| wt.info.path.clone())
        .unwrap_or_else(|| cwd.clone());

    let result = multiprocess::run(&cli, &effective_cwd, &config).await;

    // Clean up worktree: remove if no changes, keep otherwise.
    // Note: If the process is killed by SIGKILL or panics, this cleanup won't run.
    // Stale worktrees are caught by `cleanup_stale_worktrees()` on the next startup.
    if let Some(wt) = worktree {
        cleanup_session_worktree(&wt);
    }

    result
}

/// Holds worktree info for cleanup on exit.
struct SessionWorktree {
    info: loopal_git::WorktreeInfo,
    repo_root: PathBuf,
}

fn create_session_worktree(cwd: &std::path::Path) -> anyhow::Result<SessionWorktree> {
    let repo_root = loopal_git::repo_root(cwd)
        .ok_or_else(|| anyhow::anyhow!("--worktree requires a git repository"))?;
    let name = format!("session-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let info = loopal_git::create_worktree(&repo_root, &name)
        .map_err(|e| anyhow::anyhow!("failed to create worktree: {e}"))?;
    tracing::info!(worktree = %info.path.display(), branch = %info.branch, "session worktree created");
    Ok(SessionWorktree { info, repo_root })
}

fn cleanup_session_worktree(wt: &SessionWorktree) {
    if loopal_git::cleanup_if_clean(&wt.repo_root, &wt.info) {
        tracing::info!("session worktree removed (no changes)");
    } else {
        tracing::info!(
            worktree = %wt.info.path.display(),
            "worktree has changes, keeping for manual review"
        );
    }
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
