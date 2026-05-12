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
fn unknown_permission_falls_back_to_ask_any_write() {
    let cli = parse(&["--permission", "wat"]);
    let mut settings = loopal_config::Settings::default();
    cli.apply_overrides(&mut settings);
    assert!(matches!(
        settings.permission_mode,
        loopal_tool_api::PermissionMode::AskAnyWrite
    ));
}

#[test]
fn rejects_legacy_decision_auto() {
    // `auto` was the old name for Classifier; no longer accepted.
    let cli = parse(&["--decision", "auto"]);
    let mut settings = loopal_config::Settings {
        decision_mode: loopal_decision_api::DecisionMode::Manual,
        ..Default::default()
    };
    cli.apply_overrides(&mut settings);
    assert!(
        matches!(
            settings.decision_mode,
            loopal_decision_api::DecisionMode::Manual
        ),
        "legacy --decision=auto must NOT overwrite settings (parse fails silently)"
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
fn invalid_decision_leaves_settings_unchanged() {
    let cli = parse(&["--decision", "magic"]);
    let mut settings = loopal_config::Settings {
        decision_mode: loopal_decision_api::DecisionMode::Classifier,
        ..Default::default()
    };
    cli.apply_overrides(&mut settings);
    assert!(
        matches!(
            settings.decision_mode,
            loopal_decision_api::DecisionMode::Classifier
        ),
        "invalid --decision must not overwrite existing Classifier setting"
    );
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
