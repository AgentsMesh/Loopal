use tokio::sync::oneshot;

use crate::cli::Cli;

use super::typestate::{HubAliveInfo, HubBuilt, RootPending};

pub use super::typestate::BootstrapContext;

pub async fn prepare_hub_and_agent(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
) -> anyhow::Result<RootPending> {
    prepare_hub_and_agent_with_alive(cli, cwd, config, None).await
}

pub async fn prepare_hub_and_agent_with_alive(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    alive_tx: Option<oneshot::Sender<HubAliveInfo>>,
) -> anyhow::Result<RootPending> {
    let bs = HubBuilt::new(cwd, config).await?;
    let bs = bs.bind_listener().await?;
    if let Some(tx) = alive_tx {
        let _ = tx.send(bs.alive_info());
    }
    let bs = bs.register_handlers(cli).await?;
    let bs = bs.spawn_agent_process().await?;
    let bs = bs.start_event_loop();
    bs.install_workflow_runtime(&config.settings.workflow).await
}

pub async fn start_prepared_hub_and_agent(
    prepared: RootPending,
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> anyhow::Result<BootstrapContext> {
    let params = build_start_params(cli, cwd, config, resume);
    let bs = prepared.start_root_agent(params).await?;
    Ok(bs.into_context())
}

fn build_start_params(
    cli: &Cli,
    cwd: &std::path::Path,
    config: &loopal_config::ResolvedConfig,
    resume: Option<&str>,
) -> loopal_agent_client::StartAgentParams {
    let mode_str = if cli.child.plan { "plan" } else { "act" };
    let prompt = if cli.prompt.is_empty() {
        None
    } else {
        Some(cli.prompt.join(" "))
    };
    let lifecycle_str = if cli.child.ephemeral {
        Some("ephemeral".to_string())
    } else if prompt.is_some()
        && config.settings.workflow.execution_enabled
        && config.settings.workflow.policy != loopal_config::OrchestrationPolicy::Off
    {
        Some("workflow_ephemeral".to_string())
    } else {
        None
    };
    let permission_mode = cli.child.permission.as_ref().map(|p| {
        if p == "yolo" {
            "bypass".to_string()
        } else {
            p.clone()
        }
    });
    loopal_agent_client::StartAgentParams {
        cwd: cwd.to_path_buf(),
        model: Some(config.settings.model.clone()),
        mode: Some(mode_str.to_string()),
        prompt,
        permission_mode,
        decision_mode: cli.child.decision.clone(),
        no_sandbox: cli.child.no_sandbox,
        sandbox_policy: Some(config.settings.sandbox.policy.to_string()),
        session_id: None,
        workflow_permission_causation: None,
        workflow_attempt_capability: None,
        workflow_completion_result_limit: None,
        resume: resume.map(String::from),
        lifecycle: lifecycle_str,
        agent_type: None,
        depth: None,
        fork_context: None,
    }
}

#[cfg(test)]
#[path = "tests/hub_bootstrap_tests.rs"]
mod tests;
