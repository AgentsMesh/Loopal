use crate::cli::DesktopServeArgs;
use serde::Serialize;

const UNBORN_HEAD: &str = "UNBORN";

pub async fn dispatch(matches: &clap::ArgMatches, cwd: &std::path::Path) -> anyhow::Result<()> {
    match matches.subcommand() {
        Some(("serve", matches)) => run(crate::cli::parse_serve_args(matches), cwd).await,
        Some(("inspect-directory", matches)) => {
            inspect_directory(&crate::cli::parse_inspect_args(matches).path)
        }
        Some(("prepare-worktree", matches)) => {
            let args = crate::cli::parse_prepare_args(matches);
            prepare_worktree(
                &args.path,
                &args.name,
                &args.expected_root,
                (args.expected_head != UNBORN_HEAD).then_some(args.expected_head.as_str()),
            )
        }
        Some(("cleanup-worktree", matches)) => {
            let args = crate::cli::parse_cleanup_args(matches);
            cleanup_worktree(&args.path, &args.name, &args.expected_path)
        }
        _ => unreachable!("clap requires a desktop subcommand"),
    }
}

pub async fn run(args: DesktopServeArgs, cwd: &std::path::Path) -> anyhow::Result<()> {
    let parent_pid = args.parent_pid;
    if let Some(pid) = parent_pid
        && let Err(error) = super::parent_liveness::validate(pid)
    {
        super::startup_protocol::write_desktop_error(
            parent_pid,
            "invalid_parent_process",
            error.to_string(),
        )
        .await;
        return Err(error);
    }

    // Defer the session-orphan scan until after READY so it cannot exhaust the
    // machine-handshake deadline.
    super::startup_housekeeping(cwd, true).await;

    let mut config = match loopal_config::load_config(cwd) {
        Ok(config) => config,
        Err(error) => {
            super::startup_protocol::write_desktop_error(
                parent_pid,
                "configuration_failed",
                "Loopal configuration is invalid; repair the project settings files",
            )
            .await;
            return Err(error.into());
        }
    };

    let cli = args.into_runtime_cli();
    cli.apply_overrides(&mut config.settings);
    let resume = cli.parent_only.resume.as_deref();
    super::hub_only::run_desktop(&cli, cwd, &config, resume, parent_pid).await
}

pub fn inspect_directory(path: &std::path::Path) -> anyhow::Result<()> {
    print_result(loopal_workspace::inspect_working_directory(path))
}

pub fn prepare_worktree(
    path: &std::path::Path,
    name: &str,
    expected_root: &std::path::Path,
    expected_head: Option<&str>,
) -> anyhow::Result<()> {
    print_result(loopal_workspace::prepare_worktree_directory(
        path,
        name,
        expected_root,
        expected_head,
    ))
}

pub fn cleanup_worktree(
    path: &std::path::Path,
    name: &str,
    expected_path: &std::path::Path,
) -> anyhow::Result<()> {
    print_result(loopal_workspace::cleanup_prepared_worktree(
        path,
        name,
        expected_path,
    ))
}

fn print_result<T: Serialize>(
    result: Result<T, loopal_workspace::WorkspaceError>,
) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string(&result_value(result))?);
    Ok(())
}

fn result_value<T: Serialize>(
    result: Result<T, loopal_workspace::WorkspaceError>,
) -> serde_json::Value {
    match result {
        Ok(value) => serde_json::json!({ "ok": true, "value": value }),
        Err(error) => serde_json::json!({
            "ok": false, "error": { "code": error.code, "message": error.message },
        }),
    }
}

#[cfg(test)]
mod directory_command_tests {
    use super::*;

    #[test]
    fn serializes_stable_domain_error_envelope() {
        let value = result_value::<serde_json::Value>(Err(loopal_workspace::WorkspaceError::new(
            "not_git_repository",
            "Git required",
        )));
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "not_git_repository");
        assert_eq!(value["error"]["message"], "Git required");
    }
}
