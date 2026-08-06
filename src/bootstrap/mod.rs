use clap::FromArgMatches;

use loopal_config::load_config;
use loopal_vault_api::VaultError;

use crate::cli::{Cli, build_cli};

#[path = "modes/acp.rs"]
mod acp;
#[path = "modes/attach_mode.rs"]
mod attach_mode;
#[path = "modes/desktop.rs"]
mod desktop;
#[path = "modes/hub_cli.rs"]
mod hub_cli;
#[path = "modes/hub_only.rs"]
mod hub_only;
#[path = "modes/meta_hub.rs"]
mod meta_hub;
#[path = "modes/multiprocess.rs"]
mod multiprocess;
#[path = "modes/server_mode.rs"]
mod server_mode;

#[path = "hub/attach_bridge.rs"]
mod attach_bridge;
#[path = "hub/discovery.rs"]
mod discovery;
#[path = "hub/hub_bootstrap.rs"]
mod hub_bootstrap;
#[path = "hub/hub_registration.rs"]
mod hub_registration;
#[path = "hub/hub_spawn.rs"]
mod hub_spawn;
#[path = "hub/root_view_ready.rs"]
mod root_view_ready;
#[path = "hub/token_channel/mod.rs"]
mod token_channel;
#[path = "hub/typestate/mod.rs"]
pub(crate) mod typestate;
#[path = "hub/ui_ready_gate.rs"]
mod ui_ready_gate;
#[path = "hub/uplink_bootstrap.rs"]
mod uplink_bootstrap;

#[path = "process/housekeeping.rs"]
mod housekeeping;
#[path = "process/parent_liveness.rs"]
mod parent_liveness;
#[path = "process/startup_protocol.rs"]
mod startup_protocol;

#[path = "session/sub_agent_resume.rs"]
mod sub_agent_resume;
#[path = "session/worktree_session.rs"]
mod worktree_session;

#[cfg(test)]
#[path = "tests/normalize_vault_at.rs"]
mod normalize_vault_at_test;

use worktree_session::{
    cleanup_session_worktree, create_session_worktree, print_detach_worktree_info,
    print_error_worktree_info, print_resume_info, resolve_resume_for_cwd,
};

pub(crate) use discovery::is_alive;
pub(crate) use housekeeping::abbreviate_home;
use housekeeping::{cleanup_bash_log_orphans, startup_housekeeping};

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
        Some(("desktop", desktop_matches)) => {
            return desktop::dispatch(desktop_matches, &cwd).await;
        }
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

    // Hub children skip their parent's expensive session scan so startup stays
    // within the machine-handshake deadline.
    startup_housekeeping(&cwd, cli.parent_only.hub_only).await;

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
        return meta_hub::run(bind_addr, cli.parent_only.meta_hub_parent_pid).await;
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
