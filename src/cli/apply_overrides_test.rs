use clap::Parser;

use super::*;

fn parse(args: &[&str]) -> Cli {
    let mut all = vec!["loopal"];
    all.extend_from_slice(args);
    Cli::parse_from(all)
}

#[test]
fn threads_model() {
    let cli = parse(&["--model", "haiku"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert_eq!(settings.model, "haiku");
}

#[test]
fn threads_permission_bypass() {
    let cli = parse(&["--permission", "bypass"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.permission_mode,
        loopal_tool_api::PermissionMode::Bypass
    ));
}

#[test]
fn yolo_alias_is_bypass() {
    let cli = parse(&["--permission", "yolo"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.permission_mode,
        loopal_tool_api::PermissionMode::Bypass
    ));
}

#[test]
fn rejects_unknown_permission_at_clap_layer() {
    let res = Cli::try_parse_from(["loopal", "--permission", "wat"]);
    assert!(res.is_err(), "unknown --permission must fail at clap parse");
}

#[test]
fn threads_permission_ask_dangerous() {
    let cli = parse(&["--permission", "ask_dangerous"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.permission_mode,
        loopal_tool_api::PermissionMode::AskDangerous
    ));
}

#[test]
fn threads_permission_ask_any_write() {
    let cli = parse(&["--permission", "ask_any_write"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.permission_mode,
        loopal_tool_api::PermissionMode::AskAnyWrite
    ));
}

#[test]
fn every_clap_permission_value_maps_to_enum_without_panic() {
    for v in ["bypass", "ask_dangerous", "ask_any_write", "yolo"] {
        let cli = parse(&["--permission", v]);
        let mut settings = loopal_config::Settings::default();
        cli.apply_overrides(&mut settings);
    }
}

#[test]
fn every_clap_decision_value_maps_to_enum_without_panic() {
    for v in ["manual", "classifier", "agent"] {
        let cli = parse(&["--decision", v]);
        let mut settings = loopal_config::Settings::default();
        cli.apply_overrides(&mut settings);
    }
}

#[test]
fn rejects_legacy_decision_auto() {
    let res = Cli::try_parse_from(["loopal", "--decision", "auto"]);
    assert!(
        res.is_err(),
        "legacy --decision=auto must fail at clap parse"
    );
}

#[test]
fn threads_decision_classifier_canonical_name() {
    let cli = parse(&["--decision", "classifier"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.decision_mode,
        loopal_decision_api::DecisionMode::Classifier
    ));
}

#[test]
fn threads_decision_agent() {
    let cli = parse(&["--decision", "agent"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.decision_mode,
        loopal_decision_api::DecisionMode::Agent
    ));
}

#[test]
fn threads_decision_equals_syntax() {
    // `--decision=classifier` (clap = syntax) should work identically.
    let cli = parse(&["--decision=classifier"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.decision_mode,
        loopal_decision_api::DecisionMode::Classifier
    ));
}

#[test]
fn threads_decision_manual() {
    let cli = parse(&["--decision", "manual"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.decision_mode,
        loopal_decision_api::DecisionMode::Manual
    ));
}

#[test]
fn rejects_invalid_decision_at_clap_layer() {
    let res = Cli::try_parse_from(["loopal", "--decision", "magic"]);
    assert!(res.is_err(), "invalid --decision must fail at clap parse");
}

#[test]
fn disables_sandbox() {
    let cli = parse(&["--no-sandbox"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.sandbox.policy,
        loopal_config::SandboxPolicy::Disabled
    ));
}
