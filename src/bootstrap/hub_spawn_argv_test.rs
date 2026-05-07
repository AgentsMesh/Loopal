use clap::Parser;

use super::*;
use crate::cli::Cli;

fn parse(args: &[&str]) -> Cli {
    let mut all = vec!["loopal"];
    all.extend_from_slice(args);
    Cli::parse_from(all)
}

fn argv_strs(argv: &[std::ffi::OsString]) -> Vec<String> {
    argv.iter()
        .map(|s| s.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn empty_cli_emits_no_args() {
    let cli = parse(&[]);
    assert!(build_hub_only_argv(&cli, None).is_empty());
}

#[test]
fn child_passthrough_flags_are_forwarded() {
    let cli = parse(&[
        "--model",
        "opus",
        "--permission",
        "auto",
        "--plan",
        "--no-sandbox",
        "--ephemeral",
        "--join-hub",
        "127.0.0.1:9900",
        "--hub-name",
        "h1",
    ]);
    let argv = build_hub_only_argv(&cli, None);
    let strs = argv_strs(&argv);
    for expected in &[
        "--model",
        "opus",
        "--permission",
        "auto",
        "--plan",
        "--no-sandbox",
        "--ephemeral",
        "--join-hub",
        "127.0.0.1:9900",
        "--hub-name",
        "h1",
    ] {
        assert!(
            strs.iter().any(|s| s == expected),
            "expected {expected:?} in {strs:?}"
        );
    }
}

#[test]
fn parent_only_flags_are_not_forwarded() {
    let cli = parse(&["--worktree", "--server", "--no-sandbox"]);
    let strs = argv_strs(&build_hub_only_argv(&cli, None));
    assert!(!strs.iter().any(|s| s == "--worktree"));
    assert!(!strs.iter().any(|s| s == "--server"));
    assert!(strs.iter().any(|s| s == "--no-sandbox"));
}

#[test]
fn resolved_resume_id_is_appended_when_some() {
    let cli = parse(&["--no-sandbox"]);
    let strs = argv_strs(&build_hub_only_argv(&cli, Some("session-xyz")));
    assert!(
        strs.windows(2)
            .any(|w| w[0] == "--resume" && w[1] == "session-xyz")
    );
}

#[test]
fn resume_is_omitted_when_none_even_if_cli_had_resume_flag() {
    let cli = parse(&["--resume", "session-from-cli"]);
    let strs = argv_strs(&build_hub_only_argv(&cli, None));
    assert!(
        !strs.iter().any(|s| s == "--resume"),
        "child argv should not contain bare --resume; parent must resolve and pass via the resume \
         arg. Got {strs:?}"
    );
}

#[test]
fn prompt_words_are_forwarded_in_order() {
    let cli = parse(&["fix", "the", "bug"]);
    let strs = argv_strs(&build_hub_only_argv(&cli, None));
    assert_eq!(strs, vec!["fix", "the", "bug"]);
}

#[test]
fn resume_id_precedes_prompt_in_output() {
    let cli = parse(&["fix", "bug"]);
    let strs = argv_strs(&build_hub_only_argv(&cli, Some("sess-1")));
    let resume_pos = strs
        .iter()
        .position(|s| s == "--resume")
        .expect("resume present");
    let prompt_pos = strs
        .iter()
        .position(|s| s == "fix")
        .expect("prompt present");
    assert!(
        resume_pos < prompt_pos,
        "resume must precede prompt in {strs:?}"
    );
}
