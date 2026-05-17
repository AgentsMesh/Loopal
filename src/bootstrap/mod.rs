use clap::FromArgMatches;

use loopal_config::load_config;
use loopal_vault_api::VaultError;

use crate::cli::{Cli, build_cli};

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

#[cfg(test)]
mod normalize_vault_at_test;

use worktree_session::{
    cleanup_session_worktree, create_session_worktree, print_detach_worktree_info,
    print_error_worktree_info, print_resume_info, resolve_resume_for_cwd,
};

pub(crate) use discovery::is_alive;

pub(crate) fn normalize_vault_at_syntax(mut args: Vec<String>) -> Result<Vec<String>, VaultError> {
    let Some(first) = args.get(1).cloned() else {
        return Ok(args);
    };
    let Some(rest) = first.strip_prefix("vault@") else {
        return Ok(args);
    };
    loopal_vault_age::cli::validate_vault_name(rest)?;
    args[1] = "vault".to_string();
    args.insert(2, "--name".to_string());
    args.insert(3, rest.to_string());
    Ok(args)
}

pub async fn run() -> anyhow::Result<()> {
    let raw_args = match normalize_vault_at_syntax(std::env::args().collect()) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let cwd = std::env::current_dir()?;

    let matches = build_cli().get_matches_from(raw_args);

    match matches.subcommand() {
        Some(("vault", sub)) => {
            std::process::exit(loopal_vault_age::cli::dispatch_vault(sub, &cwd).await);
        }
        Some(("vaults", sub)) => {
            std::process::exit(loopal_vault_age::cli::dispatch_vaults(sub, &cwd).await);
        }
        _ => {}
    }

    let cli = Cli::from_arg_matches(&matches).expect(
        "matches just produced by build_cli().get_matches_from; from_arg_matches cannot fail",
    );

    loopal_config::housekeeping::startup_cleanup();
    if let Some(repo_root) = loopal_git::repo_root(&cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }
    discovery::cleanup_stale();
    // reason: orphan cleanup must run in every process entry, not just
    // multiprocess parent. daemon modes (serve / hub-only / acp) own
    // sessions too and would otherwise accumulate tmp dirs indefinitely.
    // Cost is a single fs read_dir + HashSet compare — negligible.
    cleanup_bash_log_orphans().await;

    let mut config = load_config(&cwd)?;
    cli.apply_overrides(&mut config.settings);

    if cli.parent_only.list_hubs {
        hub_cli::run_list_hubs();
        return Ok(());
    }
    if let Some(pid) = cli.parent_only.kill_hub {
        return hub_cli::run_kill_hub(pid).await;
    }
    if let Some(pid) = cli.parent_only.attach_hub_pid {
        return hub_cli::run_attach_pid(&cwd, &config, pid).await;
    }

    if let Some(ref bind_addr) = cli.parent_only.meta_hub {
        return meta_hub::run(bind_addr).await;
    }

    if cli.parent_only.hub_only {
        let resume = match cli.resume_intent() {
            Some(crate::cli::ResumeIntent::Specific(id)) => Some(id),
            _ => None,
        };
        return hub_only::run(&cli, &cwd, &config, resume.as_deref()).await;
    }

    if let Some(ref hub_addr) = cli.parent_only.attach_hub {
        return attach_mode::run(&cli, &cwd, &config, hub_addr).await;
    }

    if cli.parent_only.acp {
        return acp::run(&cli, &cwd, &config).await;
    }

    if cli.parent_only.serve {
        let test_provider = cli
            .parent_only
            .test_provider
            .clone()
            .or_else(|| std::env::var("LOOPAL_TEST_PROVIDER").ok());
        if let Some(path) = test_provider {
            return loopal_agent_server::run_agent_server_with_mock(&path).await;
        }
        return loopal_agent_server::run_agent_server().await;
    }

    if cli.parent_only.server {
        return server_mode::run(&cli, &cwd, &config).await;
    }

    let worktree = if cli.parent_only.worktree {
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

// reason: any `$TMPDIR/loopal/{id}/` dir whose id is not in
// `~/.loopal/sessions/` is an orphan left by a previous crash. Clean it on
// every startup so the tmp dir doesn't grow indefinitely on long-lived
// machines. Failure is non-fatal (warn only).
async fn cleanup_bash_log_orphans() {
    let live_sessions: std::collections::HashSet<String> = match loopal_storage::SessionStore::new()
    {
        Ok(store) => store
            .list_sessions()
            .map(|ss| ss.into_iter().map(|s| s.id).collect())
            .unwrap_or_default(),
        Err(_) => std::collections::HashSet::new(),
    };
    loopal_backend::cleanup_orphans(&live_sessions).await;
}
