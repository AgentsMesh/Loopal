use clap::Parser;

use loopal_config::load_config;

use crate::cli::Cli;

mod acp;
mod attach_bridge;
mod attach_mode;
mod discovery;
mod hub_bootstrap;
mod hub_cli;
mod hub_only;
mod hub_spawn;
mod meta_hub;
mod multiprocess;
mod server_mode;
mod sub_agent_resume;
mod token_channel;
mod uplink_bootstrap;
mod worktree_session;

use worktree_session::{
    cleanup_session_worktree, create_session_worktree, print_detach_worktree_info,
    print_error_worktree_info, print_resume_info, resolve_resume_for_cwd,
};

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    loopal_config::housekeeping::startup_cleanup();
    if let Some(repo_root) = loopal_git::repo_root(&cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }
    discovery::cleanup_stale();

    let mut config = load_config(&cwd)?;
    cli.apply_overrides(&mut config.settings);

    if cli.list_hubs {
        hub_cli::run_list_hubs();
        return Ok(());
    }
    if let Some(pid) = cli.kill_hub {
        return hub_cli::run_kill_hub(pid).await;
    }
    if let Some(pid) = cli.attach_hub_pid {
        return hub_cli::run_attach_pid(&cwd, &config, pid).await;
    }

    if let Some(ref bind_addr) = cli.meta_hub {
        return meta_hub::run(bind_addr).await;
    }

    if cli.hub_only {
        let resume = match cli.resume_intent() {
            Some(crate::cli::ResumeIntent::Specific(id)) => Some(id),
            _ => None,
        };
        return hub_only::run(&cli, &cwd, &config, resume.as_deref()).await;
    }

    if let Some(ref hub_addr) = cli.attach_hub {
        return attach_mode::run(&cli, &cwd, &config, hub_addr).await;
    }

    if cli.acp {
        return acp::run(&cli, &cwd, &config).await;
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

    if cli.server {
        return server_mode::run(&cli, &cwd, &config).await;
    }

    let worktree = if cli.worktree {
        Some(create_session_worktree(&cwd)?)
    } else {
        None
    };
    let effective_cwd = worktree
        .as_ref()
        .map(|wt| wt.info.path.clone())
        .unwrap_or_else(|| cwd.clone());

    let resume_session_id = match cli.resume_intent() {
        None => None,
        Some(crate::cli::ResumeIntent::Specific(id)) => Some(id),
        Some(crate::cli::ResumeIntent::Latest) => resolve_resume_for_cwd(&effective_cwd),
    };

    let result =
        multiprocess::run(&cli, &effective_cwd, &config, resume_session_id.as_deref()).await;

    let worktree_kept = match (worktree.as_ref(), &result) {
        (Some(wt), Ok(None)) => Some(wt),
        (Some(wt), _) if !cleanup_session_worktree(wt) => Some(wt),
        _ => None,
    };
    match &result {
        Ok(Some(session_id)) => print_resume_info(session_id, worktree_kept),
        Ok(None) => print_detach_worktree_info(worktree_kept),
        Err(_) => print_error_worktree_info(worktree_kept),
    }

    result.map(|_| ())
}

pub(crate) fn abbreviate_home(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = path.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}
