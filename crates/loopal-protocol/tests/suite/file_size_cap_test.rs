//! Compile-time enforcement of CLAUDE.md's 200-line cap for protocol sources.

const EVENT_PAYLOAD: &str = include_str!("../../src/event_payload.rs");
const EVENT_SUMMARY: &str = include_str!("../../src/event_summary.rs");
const CONTROL: &str = include_str!("../../src/control.rs");
const WORKFLOW_FILES: &[(&str, &str)] = &[
    (
        "capability.rs",
        include_str!("../../src/workflow/capability.rs"),
    ),
    ("command.rs", include_str!("../../src/workflow/command.rs")),
    ("event.rs", include_str!("../../src/workflow/event.rs")),
    ("ids.rs", include_str!("../../src/workflow/ids.rs")),
    ("mod.rs", include_str!("../../src/workflow/mod.rs")),
    (
        "node_validation.rs",
        include_str!("../../src/workflow/node_validation.rs"),
    ),
    ("output.rs", include_str!("../../src/workflow/output.rs")),
    ("planner.rs", include_str!("../../src/workflow/planner.rs")),
    (
        "planner_policy.rs",
        include_str!("../../src/workflow/planner_policy.rs"),
    ),
    (
        "planner_schema.rs",
        include_str!("../../src/workflow/planner_schema.rs"),
    ),
    ("reducer.rs", include_str!("../../src/workflow/reducer.rs")),
    (
        "reducer_attempt.rs",
        include_str!("../../src/workflow/reducer_attempt.rs"),
    ),
    (
        "reducer_cancel.rs",
        include_str!("../../src/workflow/reducer_cancel.rs"),
    ),
    (
        "reducer_graph.rs",
        include_str!("../../src/workflow/reducer_graph.rs"),
    ),
    (
        "reducer_stop.rs",
        include_str!("../../src/workflow/reducer_stop.rs"),
    ),
    (
        "reducer_transition.rs",
        include_str!("../../src/workflow/reducer_transition.rs"),
    ),
    ("request.rs", include_str!("../../src/workflow/request.rs")),
    ("retry.rs", include_str!("../../src/workflow/retry.rs")),
    (
        "schema_validation.rs",
        include_str!("../../src/workflow/schema_validation.rs"),
    ),
    ("spec.rs", include_str!("../../src/workflow/spec.rs")),
    (
        "start_lookup.rs",
        include_str!("../../src/workflow/start_lookup.rs"),
    ),
    ("state.rs", include_str!("../../src/workflow/state.rs")),
    ("summary.rs", include_str!("../../src/workflow/summary.rs")),
    (
        "terminal.rs",
        include_str!("../../src/workflow/terminal.rs"),
    ),
    (
        "terminal_bounds.rs",
        include_str!("../../src/workflow/terminal_bounds.rs"),
    ),
    (
        "validation.rs",
        include_str!("../../src/workflow/validation.rs"),
    ),
];

const HARD_LIMIT: usize = 200;

fn assert_under_cap(name: &str, content: &str) {
    let lines = content.lines().count();
    assert!(
        lines <= HARD_LIMIT,
        "{name} is {lines} lines (limit: {HARD_LIMIT})"
    );
}

#[test]
fn established_protocol_files_stay_under_line_cap() {
    for (name, content) in [
        ("event_payload.rs", EVENT_PAYLOAD),
        ("event_summary.rs", EVENT_SUMMARY),
        ("control.rs", CONTROL),
    ] {
        assert_under_cap(name, content);
    }
}

#[test]
fn every_workflow_source_is_listed_and_under_line_cap() {
    let manifest = include_str!("../../src/workflow/mod.rs");
    let declared = manifest
        .lines()
        .filter(|line| line.starts_with("mod "))
        .count();
    assert_eq!(
        declared,
        WORKFLOW_FILES.len() - 1,
        "update WORKFLOW_FILES for every module"
    );
    for (name, content) in WORKFLOW_FILES {
        assert_under_cap(name, content);
    }
}
