use clap::{Args, Command, FromArgMatches as _};

use crate::cli::{ChildPassthroughArgs, Cli, ParentOnlyArgs};

/// Arguments for the stable Desktop Host entry point.
///
/// The Desktop starts with no prompt so its UI client can register before any
/// Agent turn begins. Prompts are routed over the Hub protocol after READY.
#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct DesktopServeArgs {
    #[command(flatten)]
    pub child: ChildPassthroughArgs,

    /// Resume a specific persisted Loopal session
    #[arg(long, value_name = "SESSION_ID")]
    pub resume: Option<String>,

    /// Exit and drain the Agent process when this Desktop parent exits
    #[arg(long, value_name = "PID", value_parser = crate::cli::parse_pid)]
    pub parent_pid: Option<u32>,
}

pub fn desktop_command() -> Command {
    super::desktop_directory::add_directory_commands(
        Command::new("desktop")
            .about("Run Loopal as a desktop application backend")
            .subcommand_required(true)
            .arg_required_else_help(true)
            .subcommand(DesktopServeArgs::augment_args(Command::new("serve").about(
                "Start the versioned Desktop Host protocol on stdout and a local Hub transport",
            ))),
    )
}

pub fn parse_serve_args(matches: &clap::ArgMatches) -> DesktopServeArgs {
    DesktopServeArgs::from_arg_matches(matches)
        .expect("matches produced by desktop_command; DesktopServeArgs must parse")
}

impl DesktopServeArgs {
    /// Adapt the stable command to the existing Hub bootstrap without making
    /// the Desktop protocol depend on hidden `--hub-only` argv.
    pub fn into_runtime_cli(self) -> Cli {
        Cli {
            child: self.child,
            parent_only: ParentOnlyArgs {
                resume: self.resume,
                hub_only: true,
                ..ParentOnlyArgs::default()
            },
            prompt: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::{build_cli, parse_inspect_args, parse_prepare_args};

    fn parse(argv: &[&str]) -> DesktopServeArgs {
        let matches = build_cli().try_get_matches_from(argv).expect("parse argv");
        let (_, desktop) = matches.subcommand().expect("desktop selected");
        let (_, serve) = desktop.subcommand().expect("serve selected");
        parse_serve_args(serve)
    }

    #[test]
    fn serve_parses_parent_resume_and_runtime_overrides() {
        let args = parse(&[
            "loopal",
            "desktop",
            "serve",
            "--parent-pid",
            "42",
            "--resume",
            "session-1",
            "--model",
            "test-model",
            "--permission",
            "ask_dangerous",
            "--decision",
            "manual",
            "--plan",
            "--no-sandbox",
            "--ephemeral",
        ]);

        assert_eq!(args.parent_pid, Some(42));
        assert_eq!(args.resume.as_deref(), Some("session-1"));
        assert_eq!(args.child.model.as_deref(), Some("test-model"));
        assert_eq!(args.child.permission.as_deref(), Some("ask_dangerous"));
        assert_eq!(args.child.decision.as_deref(), Some("manual"));
        assert!(args.child.plan);
        assert!(args.child.no_sandbox);
        assert!(args.child.ephemeral);
    }

    #[test]
    fn serve_rejects_zero_parent_pid() {
        let result =
            build_cli().try_get_matches_from(["loopal", "desktop", "serve", "--parent-pid", "0"]);
        assert!(result.is_err());
    }

    #[test]
    fn serve_requires_resume_value() {
        let result = build_cli().try_get_matches_from(["loopal", "desktop", "serve", "--resume"]);
        assert!(result.is_err());
    }

    #[test]
    fn runtime_adapter_never_injects_a_startup_prompt() {
        let cli =
            parse(&["loopal", "desktop", "serve", "--resume", "session-1"]).into_runtime_cli();

        assert!(cli.parent_only.hub_only);
        assert_eq!(cli.parent_only.resume.as_deref(), Some("session-1"));
        assert!(cli.prompt.is_empty());
    }

    #[test]
    fn desktop_requires_a_nested_subcommand() {
        assert!(
            build_cli()
                .try_get_matches_from(["loopal", "desktop"])
                .is_err()
        );
    }

    #[test]
    fn hidden_directory_commands_parse_paths_and_names() {
        let matches = build_cli()
            .try_get_matches_from([
                "loopal",
                "desktop",
                "inspect-directory",
                "--path",
                "/tmp/project",
            ])
            .unwrap();
        let (_, desktop) = matches.subcommand().unwrap();
        let (_, inspect) = desktop.subcommand().unwrap();
        assert_eq!(
            parse_inspect_args(inspect).path,
            PathBuf::from("/tmp/project")
        );

        let matches = build_cli()
            .try_get_matches_from([
                "loopal",
                "desktop",
                "prepare-worktree",
                "--path",
                "/tmp/project",
                "--name",
                "desktop-1",
                "--expected-root",
                "/tmp/project",
                "--expected-head",
                "UNBORN",
            ])
            .unwrap();
        let (_, desktop) = matches.subcommand().unwrap();
        let (_, prepare) = desktop.subcommand().unwrap();
        let args = parse_prepare_args(prepare);
        assert_eq!(args.path, PathBuf::from("/tmp/project"));
        assert_eq!(args.name, "desktop-1");
        assert_eq!(args.expected_root, PathBuf::from("/tmp/project"));
        assert_eq!(args.expected_head, "UNBORN");
    }
}
