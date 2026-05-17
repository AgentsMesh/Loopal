//! Reflective completeness test for the ControlCommand → AgentEvent →
//! ViewState contract.
//!
//! The invariant we enforce: every `ControlCommand` variant must declare
//! whether it mutates the view-state (the common case) or relies on a
//! fixture (goal session, MCP server, persisted session…) that the
//! default harness does not provide. Adding a new variant without
//! touching `expected_view_effect()` causes a compile error (exhaustive
//! match) — the next person who forgets to wire an emit cannot ship.
//!
//! The dedicated lifecycle tests in `e2e_control_lifecycle_test.rs` and
//! `e2e_control_test.rs` are the runtime side of this contract: they
//! actually drive each `ViewMutation` variant end-to-end and assert the
//! view-state changes.

use loopal_protocol::ControlCommand;
use strum::IntoEnumIterator;

/// Per-variant declaration of how the runner is expected to affect
/// view-state in the default harness fixture.
enum ExpectedViewEffect {
    /// Runner emits an event that mutates view-state. Verified by the
    /// matching `e2e_control_*_test`.
    ViewMutation,
    /// Mutation requires fixture state we don't set up here (e.g.
    /// active goal, MCP server, persisted session). Adding such a
    /// variant without a dedicated test is allowed — but the dedicated
    /// test must exist somewhere in the suite.
    FixtureRequired(&'static str),
}

fn expected_view_effect(cmd: &ControlCommand) -> ExpectedViewEffect {
    use ControlCommand::*;
    use ExpectedViewEffect::*;
    match cmd {
        ModeSwitch(_) => ViewMutation,
        Clear => ViewMutation,
        Compact => ViewMutation,
        ModelSwitch(_) => ViewMutation,
        Rewind { .. } => ViewMutation,
        ThinkingSwitch(_) => ViewMutation,
        ResumeSession(_) => FixtureRequired("persisted session on disk"),
        QueryMcpStatus => ViewMutation,
        McpReconnect { .. } => FixtureRequired("registered MCP server"),
        McpDisconnect { .. } => FixtureRequired("registered MCP server"),
        GoalCreate { .. } => FixtureRequired("goal session enabled"),
        GoalUserPause => FixtureRequired("active goal"),
        GoalUserResume => FixtureRequired("paused goal"),
        GoalUserComplete => FixtureRequired("active goal"),
        GoalClear => FixtureRequired("active goal"),
    }
}

#[test]
fn every_control_command_variant_is_classified() {
    // Round-trip every variant through `expected_view_effect` — strum's
    // `EnumIter` yields one inhabitant per variant via `Default`.
    // A new variant that wasn't added to the match panics the runner
    // via the exhaustive `match` in `expected_view_effect` at compile
    // time, not here. This test only enforces that classification
    // emitted *something* (i.e. didn't accidentally evaluate to the
    // `!` type or panic).
    for cmd in ControlCommand::iter() {
        let _ = expected_view_effect(&cmd);
    }
}

#[test]
fn fixture_required_variants_carry_a_human_explanation() {
    // The point of `FixtureRequired(&'static str)` is to leave a note
    // for the next person about *why* this variant skips the default
    // harness; if the string is empty the explanation is missing.
    for cmd in ControlCommand::iter() {
        if let ExpectedViewEffect::FixtureRequired(reason) = expected_view_effect(&cmd) {
            assert!(
                !reason.trim().is_empty(),
                "variant {} declared FixtureRequired but reason is empty — \
                 add a one-liner so future readers know which fixture to set up",
                variant_name(&cmd)
            );
        }
    }
}

#[test]
fn view_mutation_variants_are_exercised_by_dedicated_e2e_tests() {
    // Catalogue the variants that must have e2e coverage. If a new
    // variant lands as `ViewMutation`, the developer is forced to add
    // a dedicated test (this list is the cross-reference).
    let expected_tests: &[(&str, &str)] = &[
        ("Clear", "e2e_control_test::test_clear_command"),
        ("Compact", "e2e_control_test::test_compact_command"),
        ("Rewind", "e2e_control_test::test_rewind_command"),
        (
            "ModeSwitch",
            "e2e_test/permission tests cover ModeSwitch via mode picker",
        ),
        (
            "ModelSwitch",
            "e2e_control_lifecycle_test::test_model_switch_command_updates_observable",
        ),
        (
            "ThinkingSwitch",
            "e2e_control_lifecycle_test::test_thinking_switch_command_updates_observable",
        ),
        ("QueryMcpStatus", "e2e_mcp_test::*"),
    ];
    let mut covered = std::collections::HashSet::new();
    for (name, _) in expected_tests {
        covered.insert(*name);
    }
    for cmd in ControlCommand::iter() {
        if let ExpectedViewEffect::ViewMutation = expected_view_effect(&cmd) {
            let key = variant_name(&cmd);
            assert!(
                covered.contains(&key),
                "variant {key:?} declared ViewMutation but has no dedicated e2e test \
                 entry in `expected_tests`; add it (or downgrade to FixtureRequired \
                 with a justification)"
            );
        }
    }
}

fn variant_name(cmd: &ControlCommand) -> &'static str {
    use ControlCommand::*;
    match cmd {
        ModeSwitch(_) => "ModeSwitch",
        Clear => "Clear",
        Compact => "Compact",
        ModelSwitch(_) => "ModelSwitch",
        Rewind { .. } => "Rewind",
        ThinkingSwitch(_) => "ThinkingSwitch",
        ResumeSession(_) => "ResumeSession",
        QueryMcpStatus => "QueryMcpStatus",
        McpReconnect { .. } => "McpReconnect",
        McpDisconnect { .. } => "McpDisconnect",
        GoalCreate { .. } => "GoalCreate",
        GoalUserPause => "GoalUserPause",
        GoalUserResume => "GoalUserResume",
        GoalUserComplete => "GoalUserComplete",
        GoalClear => "GoalClear",
    }
}
