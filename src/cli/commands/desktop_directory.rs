use std::path::PathBuf;

use clap::{Args, Command, FromArgMatches as _};

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct DesktopInspectDirectoryArgs {
    #[arg(long, value_name = "PATH")]
    pub path: PathBuf,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct DesktopPrepareWorktreeArgs {
    #[arg(long, value_name = "PATH")]
    pub path: PathBuf,
    #[arg(long, value_name = "NAME")]
    pub name: String,
    #[arg(long, value_name = "PATH")]
    pub expected_root: PathBuf,
    #[arg(long, value_name = "OID")]
    pub expected_head: String,
}

#[derive(Args, Debug, Clone, PartialEq, Eq)]
pub struct DesktopCleanupWorktreeArgs {
    #[arg(long, value_name = "PATH")]
    pub path: PathBuf,
    #[arg(long, value_name = "NAME")]
    pub name: String,
    #[arg(long, value_name = "PATH")]
    pub expected_path: PathBuf,
}

pub fn add_directory_commands(command: Command) -> Command {
    command
        .subcommand(DesktopInspectDirectoryArgs::augment_args(
            Command::new("inspect-directory").hide(true),
        ))
        .subcommand(DesktopPrepareWorktreeArgs::augment_args(
            Command::new("prepare-worktree").hide(true),
        ))
        .subcommand(DesktopCleanupWorktreeArgs::augment_args(
            Command::new("cleanup-worktree").hide(true),
        ))
}

pub fn parse_inspect_args(matches: &clap::ArgMatches) -> DesktopInspectDirectoryArgs {
    DesktopInspectDirectoryArgs::from_arg_matches(matches)
        .expect("matches produced by desktop_command")
}

pub fn parse_prepare_args(matches: &clap::ArgMatches) -> DesktopPrepareWorktreeArgs {
    DesktopPrepareWorktreeArgs::from_arg_matches(matches)
        .expect("matches produced by desktop_command")
}

pub fn parse_cleanup_args(matches: &clap::ArgMatches) -> DesktopCleanupWorktreeArgs {
    DesktopCleanupWorktreeArgs::from_arg_matches(matches)
        .expect("matches produced by desktop_command")
}
