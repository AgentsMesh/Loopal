use std::collections::HashSet;

use clap::{Args, Parser};

use super::*;

fn full_child() -> ChildPassthroughArgs {
    ChildPassthroughArgs {
        model: Some("opus".into()),
        permission: Some("ask_dangerous".into()),
        decision: Some("classifier".into()),
        plan: true,
        no_sandbox: true,
        ephemeral: true,
        join_hub: Some("127.0.0.1:9900".into()),
        hub_name: Some("h1".into()),
    }
}

#[test]
fn to_args_covers_every_clap_long_name() {
    let cmd = ChildPassthroughArgs::augment_args(clap::Command::new("test"));
    let schema_longs: HashSet<String> = cmd
        .get_arguments()
        .filter_map(|a| a.get_long().map(String::from))
        .collect();

    let argv = full_child().to_args();
    let emitted_longs: HashSet<String> = argv
        .iter()
        .filter_map(|os| os.to_str())
        .filter_map(|s| s.strip_prefix("--").map(String::from))
        .collect();

    let missing: Vec<&String> = schema_longs.difference(&emitted_longs).collect();
    assert!(
        missing.is_empty(),
        "ChildPassthroughArgs::to_args missed flags {missing:?}. \
         Add them to to_args (or remove from struct)."
    );

    let unexpected: Vec<&String> = emitted_longs.difference(&schema_longs).collect();
    assert!(
        unexpected.is_empty(),
        "ChildPassthroughArgs::to_args emitted unknown flags {unexpected:?}."
    );
}

#[test]
fn round_trip_full() {
    let original = full_child();
    let mut argv: Vec<std::ffi::OsString> = vec!["loopal".into()];
    argv.extend(original.to_args());
    let parsed = Cli::parse_from(argv);
    assert_eq!(parsed.child, original);
}

#[test]
fn round_trip_default_emits_nothing() {
    let original = ChildPassthroughArgs::default();
    let argv = original.to_args();
    assert!(
        argv.is_empty(),
        "default ChildPassthroughArgs::to_args must emit no flags, got {argv:?}"
    );
}

#[test]
fn round_trip_each_field_individually() {
    let cases: Vec<ChildPassthroughArgs> = vec![
        ChildPassthroughArgs {
            model: Some("sonnet".into()),
            ..Default::default()
        },
        ChildPassthroughArgs {
            permission: Some("bypass".into()),
            ..Default::default()
        },
        ChildPassthroughArgs {
            decision: Some("classifier".into()),
            ..Default::default()
        },
        ChildPassthroughArgs {
            plan: true,
            ..Default::default()
        },
        ChildPassthroughArgs {
            no_sandbox: true,
            ..Default::default()
        },
        ChildPassthroughArgs {
            ephemeral: true,
            ..Default::default()
        },
        ChildPassthroughArgs {
            join_hub: Some("127.0.0.1:9900".into()),
            ..Default::default()
        },
        ChildPassthroughArgs {
            hub_name: Some("worker-1".into()),
            ..Default::default()
        },
    ];

    for case in cases {
        let mut argv: Vec<std::ffi::OsString> = vec!["loopal".into()];
        argv.extend(case.to_args());
        let parsed = Cli::parse_from(argv);
        assert_eq!(parsed.child, case, "round-trip mismatch for {case:?}");
    }
}

#[test]
fn join_hub_propagates_through_argv_round_trip() {
    let parent = Cli::parse_from([
        "loopal",
        "--join-hub",
        "127.0.0.1:9900",
        "--hub-name",
        "worker-a",
        "--no-sandbox",
    ]);

    let mut child_argv: Vec<std::ffi::OsString> = vec!["loopal".into()];
    child_argv.extend(parent.child.to_args());
    let child = Cli::parse_from(child_argv);

    assert_eq!(child.child.join_hub.as_deref(), Some("127.0.0.1:9900"));
    assert_eq!(child.child.hub_name.as_deref(), Some("worker-a"));
    assert!(child.child.no_sandbox);
}

#[test]
fn parent_only_flags_do_not_leak_into_child() {
    let parent = Cli::parse_from([
        "loopal",
        "--worktree",
        "--server",
        "--meta-hub",
        "127.0.0.1:1",
        "--join-hub",
        "127.0.0.1:9900",
    ]);

    let argv_strs: Vec<String> = parent
        .child
        .to_args()
        .iter()
        .filter_map(|s| s.to_str().map(String::from))
        .collect();

    for forbidden in &[
        "--worktree",
        "--server",
        "--acp",
        "--serve",
        "--meta-hub",
        "--attach-hub",
        "--hub-token",
        "--hub-only",
        "--require-ui-ready",
        "--list-hubs",
        "--attach-hub-pid",
        "--kill-hub",
        "--resume",
        "--test-provider",
    ] {
        assert!(
            !argv_strs.iter().any(|s| s == forbidden),
            "child.to_args leaked parent-only flag {forbidden}: {argv_strs:?}"
        );
    }
}
