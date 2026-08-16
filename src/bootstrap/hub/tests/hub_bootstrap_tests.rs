use std::path::Path;

use crate::cli::{ChildPassthroughArgs, Cli, ParentOnlyArgs};

use super::build_start_params;

fn config(
    model: &str,
    sandbox_policy: loopal_config::SandboxPolicy,
) -> loopal_config::ResolvedConfig {
    let settings = loopal_config::Settings {
        model: model.into(),
        sandbox: loopal_config::SandboxConfig {
            policy: sandbox_policy,
            ..Default::default()
        },
        ..Default::default()
    };
    loopal_config::ResolvedConfig {
        settings,
        workflow_preset_thinking_recommendation: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        classifier_prompt: None,
        layers: Vec::new(),
        secrets: None,
    }
}

fn cli(child: ChildPassthroughArgs, prompt: &[&str]) -> Cli {
    Cli {
        child,
        parent_only: ParentOnlyArgs::default(),
        prompt: prompt.iter().map(|part| (*part).to_string()).collect(),
    }
}

#[test]
fn start_params_preserve_default_act_and_persistent_behavior() {
    let cwd = Path::new("/workspace/project");
    let config = config("test-model", loopal_config::SandboxPolicy::ReadOnly);
    let params = build_start_params(
        &cli(ChildPassthroughArgs::default(), &[]),
        cwd,
        &config,
        None,
    );

    assert_eq!(params.cwd, cwd);
    assert_eq!(params.model.as_deref(), Some("test-model"));
    assert_eq!(params.mode.as_deref(), Some("act"));
    assert!(params.prompt.is_none());
    assert!(params.permission_mode.is_none());
    assert!(params.decision_mode.is_none());
    assert!(!params.no_sandbox);
    assert_eq!(params.sandbox_policy.as_deref(), Some("read_only"));
    assert!(params.resume.is_none());
    assert!(params.lifecycle.is_none());
}

#[test]
fn start_params_bind_explicit_plan_ephemeral_and_resume_values() {
    let child = ChildPassthroughArgs {
        permission: Some("yolo".into()),
        decision: Some("classifier".into()),
        plan: true,
        no_sandbox: true,
        ephemeral: true,
        ..Default::default()
    };
    let config = config("configured-model", loopal_config::SandboxPolicy::Disabled);
    let params = build_start_params(
        &cli(child, &["review", "this"]),
        Path::new("/repo"),
        &config,
        Some("session-42"),
    );

    assert_eq!(params.mode.as_deref(), Some("plan"));
    assert_eq!(params.prompt.as_deref(), Some("review this"));
    assert_eq!(params.permission_mode.as_deref(), Some("bypass"));
    assert_eq!(params.decision_mode.as_deref(), Some("classifier"));
    assert!(params.no_sandbox);
    assert_eq!(params.sandbox_policy.as_deref(), Some("disabled"));
    assert_eq!(params.resume.as_deref(), Some("session-42"));
    assert_eq!(params.lifecycle.as_deref(), Some("ephemeral"));
    assert!(params.session_id.is_none());
    assert!(params.workflow_permission_causation.is_none());
    assert!(params.agent_type.is_none());
    assert!(params.depth.is_none());
    assert!(params.fork_context.is_none());
}

#[test]
fn start_params_keep_canonical_permission_mode() {
    let child = ChildPassthroughArgs {
        permission: Some("ask_any_write".into()),
        ..Default::default()
    };
    let config = config("test-model", loopal_config::SandboxPolicy::DefaultWrite);
    let params = build_start_params(&cli(child, &[]), Path::new("/repo"), &config, None);

    assert_eq!(params.permission_mode.as_deref(), Some("ask_any_write"));
}

#[test]
fn headless_prompt_waits_for_enabled_workflow_delivery() {
    let mut config = config("test-model", loopal_config::SandboxPolicy::ReadOnly);
    config.settings.workflow.policy = loopal_config::OrchestrationPolicy::Proactive;
    config.settings.workflow.execution_enabled = true;

    let params = build_start_params(
        &cli(ChildPassthroughArgs::default(), &["delegate", "this"]),
        Path::new("/repo"),
        &config,
        None,
    );

    assert_eq!(params.lifecycle.as_deref(), Some("workflow_ephemeral"));
}

#[test]
fn workflow_disabled_or_off_keeps_prompt_default_ephemeral() {
    let mut config = config("test-model", loopal_config::SandboxPolicy::ReadOnly);
    config.settings.workflow.policy = loopal_config::OrchestrationPolicy::Proactive;

    let disabled = build_start_params(
        &cli(ChildPassthroughArgs::default(), &["direct"]),
        Path::new("/repo"),
        &config,
        None,
    );
    assert!(disabled.lifecycle.is_none());

    config.settings.workflow.execution_enabled = true;
    config.settings.workflow.policy = loopal_config::OrchestrationPolicy::Off;
    let off = build_start_params(
        &cli(ChildPassthroughArgs::default(), &["direct"]),
        Path::new("/repo"),
        &config,
        None,
    );
    assert!(off.lifecycle.is_none());
}

#[test]
fn explicit_ephemeral_overrides_workflow_aware_lifecycle() {
    let mut config = config("test-model", loopal_config::SandboxPolicy::ReadOnly);
    config.settings.workflow.policy = loopal_config::OrchestrationPolicy::Explicit;
    config.settings.workflow.execution_enabled = true;
    let child = ChildPassthroughArgs {
        ephemeral: true,
        ..Default::default()
    };

    let params = build_start_params(
        &cli(child, &["delegate"]),
        Path::new("/repo"),
        &config,
        None,
    );

    assert_eq!(params.lifecycle.as_deref(), Some("ephemeral"));
}
