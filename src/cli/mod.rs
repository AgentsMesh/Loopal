mod args;
mod commands;
mod root;

pub(crate) use args::parse_pid;
pub use args::{ChildPassthroughArgs, ParentOnlyArgs};
pub use commands::{
    DesktopServeArgs, desktop_command, parse_cleanup_args, parse_inspect_args, parse_prepare_args,
    parse_serve_args,
};
pub use root::{Cli, ResumeIntent, build_cli};

#[cfg(test)]
#[path = "tests/apply_overrides.rs"]
mod apply_overrides_test;
#[cfg(test)]
#[path = "tests/build_cli.rs"]
mod build_cli_test;
#[cfg(test)]
#[path = "tests/child_args.rs"]
mod child_args_test;
#[cfg(test)]
#[path = "tests/resume_intent.rs"]
mod resume_intent_test;
