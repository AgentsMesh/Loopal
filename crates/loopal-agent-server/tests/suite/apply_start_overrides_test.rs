//! Regression tests for permission_mode / decision_mode independent override.
//!
//! These tests verify that specifying only one of the two config dimensions
//! does NOT affect the other — the root cause of the classifier bug where
//! `--decision classifier` alone caused permission_mode to flip from Bypass
//! to AskAnyWrite.

use loopal_config::Settings;
use loopal_decision_api::DecisionMode;
use loopal_runtime::LifecycleMode;
use loopal_tool_api::PermissionMode;

use loopal_agent_server::testing::StartParams;

fn default_start_params() -> StartParams {
    StartParams {
        cwd: None,
        model: None,
        mode: None,
        prompt: None,
        permission_mode: None,
        decision_mode: None,
        no_sandbox: false,
        resume: None,
        lifecycle: LifecycleMode::Ephemeral,
        agent_type: None,
        depth: None,
        fork_context: None,
    }
}

#[test]
fn neither_specified_preserves_settings_defaults() {
    let mut settings = Settings::default();
    let start = default_start_params();

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(settings.permission_mode, PermissionMode::Bypass);
    assert_eq!(settings.decision_mode, DecisionMode::Manual);
}

#[test]
fn only_decision_mode_preserves_permission_mode() {
    let mut settings = Settings::default();
    let mut start = default_start_params();
    start.decision_mode = Some("classifier".to_string());

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(
        settings.permission_mode,
        PermissionMode::Bypass,
        "permission_mode must remain Bypass when only decision_mode is specified"
    );
    assert_eq!(settings.decision_mode, DecisionMode::Classifier);
}

#[test]
fn only_permission_mode_preserves_decision_mode() {
    let mut settings = Settings::default();
    let mut start = default_start_params();
    start.permission_mode = Some("ask_dangerous".to_string());

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(settings.permission_mode, PermissionMode::AskDangerous);
    assert_eq!(
        settings.decision_mode,
        DecisionMode::Manual,
        "decision_mode must remain Manual when only permission_mode is specified"
    );
}

#[test]
fn both_specified_overrides_both() {
    let mut settings = Settings::default();
    let mut start = default_start_params();
    start.permission_mode = Some("ask_any_write".to_string());
    start.decision_mode = Some("classifier".to_string());

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(settings.permission_mode, PermissionMode::AskAnyWrite);
    assert_eq!(settings.decision_mode, DecisionMode::Classifier);
}

#[test]
fn invalid_permission_mode_ignored() {
    let mut settings = Settings::default();
    let mut start = default_start_params();
    start.permission_mode = Some("invalid_mode".to_string());

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(
        settings.permission_mode,
        PermissionMode::Bypass,
        "invalid permission_mode should be ignored, keeping default"
    );
}

#[test]
fn invalid_decision_mode_ignored() {
    let mut settings = Settings::default();
    let mut start = default_start_params();
    start.decision_mode = Some("invalid_decision".to_string());

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(
        settings.decision_mode,
        DecisionMode::Manual,
        "invalid decision_mode should be ignored, keeping default"
    );
}

#[test]
fn settings_file_values_preserved_when_cli_unspecified() {
    let mut settings = Settings {
        permission_mode: PermissionMode::AskDangerous,
        decision_mode: DecisionMode::Classifier,
        ..Default::default()
    };

    let start = default_start_params();

    loopal_agent_server::testing::apply_start_overrides(&mut settings, &start);

    assert_eq!(
        settings.permission_mode,
        PermissionMode::AskDangerous,
        "settings file permission_mode must be preserved when CLI unspecified"
    );
    assert_eq!(
        settings.decision_mode,
        DecisionMode::Classifier,
        "settings file decision_mode must be preserved when CLI unspecified"
    );
}
