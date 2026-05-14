use std::path::PathBuf;
use std::sync::Arc;

use loopal_agent::task_store::TaskStore;
use loopal_error::AgentOutput;
use loopal_runtime::AgentLoopParams;
use loopal_runtime::agent_loop;
use loopal_scheduler::CronScheduler;
use loopal_tool_background::SpawnNotification;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::info;

use crate::hub_frontend::HubFrontend;
use crate::params::StartParams;

pub(crate) fn parse_start_params(
    params: &serde_json::Value,
) -> anyhow::Result<(StartParams, PathBuf, loopal_runtime::LifecycleMode)> {
    let cwd_str = params["cwd"].as_str().map(String::from);
    let cwd = cwd_str
        .as_deref()
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

    let lifecycle = match params["lifecycle"].as_str() {
        Some("ephemeral") => loopal_runtime::LifecycleMode::Ephemeral,
        Some("persistent") => loopal_runtime::LifecycleMode::Persistent,
        Some(unknown) => {
            anyhow::bail!(
                "unknown lifecycle mode: '{unknown}' (expected 'ephemeral' or 'persistent')"
            );
        }
        None if params["prompt"].as_str().is_some() => loopal_runtime::LifecycleMode::Ephemeral,
        None => loopal_runtime::LifecycleMode::Persistent,
    };

    let start = StartParams {
        cwd: cwd_str,
        model: params["model"].as_str().map(String::from),
        mode: params["mode"].as_str().map(String::from),
        prompt: params["prompt"].as_str().map(String::from),
        permission_mode: params["permission_mode"].as_str().map(String::from),
        decision_mode: params["decision_mode"].as_str().map(String::from),
        no_sandbox: params["no_sandbox"].as_bool().unwrap_or(false),
        resume: params["resume"].as_str().map(String::from),
        lifecycle,
        agent_type: params["agent_type"].as_str().map(String::from),
        depth: params["depth"].as_u64().map(|v| v as u32),
        fork_context: params.get("fork_context").cloned(),
    };

    Ok((start, cwd, lifecycle))
}

pub(crate) fn spawn_agent_and_bridges(
    agent_params: AgentLoopParams,
    task_store: Arc<TaskStore>,
    scheduler: Arc<CronScheduler>,
    bg_spawn_rx: mpsc::UnboundedReceiver<SpawnNotification>,
    frontend: Arc<HubFrontend>,
) -> JoinHandle<Option<AgentOutput>> {
    let bridge_task = crate::bg_task_bridge::spawn(bg_spawn_rx, frontend.clone());
    let task_change_rx = task_store.subscribe();
    let task_bridge_task = crate::task_bridge::spawn(task_change_rx, task_store, frontend.clone());
    let cron_bridge_task = crate::cron_bridge::spawn(scheduler, frontend);

    let agent_task = tokio::spawn(async move {
        match agent_loop(agent_params).await {
            Ok(output) => {
                info!(reason = ?output.terminate_reason, "agent loop completed");
                Some(output)
            }
            Err(e) => {
                tracing::error!(error = %e, "agent loop error");
                None
            }
        }
    });

    let bridge_abort = bridge_task.abort_handle();
    let task_bridge_abort = task_bridge_task.abort_handle();
    let cron_bridge_abort = cron_bridge_task.abort_handle();
    tokio::spawn(async move {
        let result = agent_task.await;
        bridge_abort.abort();
        task_bridge_abort.abort();
        cron_bridge_abort.abort();
        result.ok().flatten()
    })
}
