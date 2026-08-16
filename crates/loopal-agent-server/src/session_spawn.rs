use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use loopal_agent::task_store::TaskStore;
use loopal_error::AgentOutput;
use loopal_runtime::AgentLoopParams;
use loopal_runtime::agent_loop;
use loopal_scheduler::CronScheduler;
use loopal_tool_background::{BackgroundTaskStore, SpawnNotification};
use tokio::sync::{broadcast, oneshot};
use tokio::task::{AbortHandle, JoinHandle};
use tracing::info;

use crate::hub_frontend::HubFrontend;
use crate::params::StartParams;

// reason: bridge graceful shutdown budget. We send a signal and give the
// bridge up to this long to drain in-flight events before falling back to
// abort. 1s covers the worst case where a per-task monitor is mid-emit.
const BRIDGE_SHUTDOWN_GRACE: Duration = Duration::from_secs(1);

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
        Some("workflow_ephemeral") => loopal_runtime::LifecycleMode::WorkflowEphemeral,
        Some(unknown) => {
            anyhow::bail!(
                "unknown lifecycle mode: '{unknown}' (expected 'ephemeral', 'persistent', or 'workflow_ephemeral')"
            );
        }
        None if params["prompt"].as_str().is_some() => loopal_runtime::LifecycleMode::Ephemeral,
        None => loopal_runtime::LifecycleMode::Persistent,
    };

    let no_sandbox = params["no_sandbox"].as_bool().unwrap_or(false);
    let sandbox_policy = match params.get("sandbox_policy") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("sandbox_policy must be a string"))?
                .parse::<loopal_config::SandboxPolicy>()
                .map_err(anyhow::Error::msg)?,
        ),
    };
    if no_sandbox
        && sandbox_policy.is_some_and(|policy| policy != loopal_config::SandboxPolicy::Disabled)
    {
        anyhow::bail!("no_sandbox conflicts with sandbox_policy");
    }

    let session_id = match params.get("session_id") {
        None | Some(serde_json::Value::Null) => None,
        Some(serde_json::Value::String(value)) => Some(
            uuid::Uuid::parse_str(value)
                .map_err(|_| anyhow::anyhow!("session_id must be a UUID"))?,
        ),
        Some(_) => anyhow::bail!("session_id must be a string"),
    };

    let workflow_permission_causation = match params.get("workflow_permission_causation") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => {
            let causation: loopal_protocol::WorkflowPermissionCausation =
                serde_json::from_value(value.clone())
                    .map_err(|_| anyhow::anyhow!("invalid workflow permission causation"))?;
            if !causation.is_valid() {
                anyhow::bail!("invalid workflow permission causation");
            }
            Some(causation)
        }
    };
    let workflow_attempt_capability = match params.get("workflow_attempt_capability") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => Some(
            serde_json::from_value(value.clone())
                .map_err(|_| anyhow::anyhow!("invalid workflow attempt capability"))?,
        ),
    };
    let workflow_completion_result_limit = match params.get("workflow_completion_result_limit") {
        None | Some(serde_json::Value::Null) => None,
        Some(value) => {
            let value = value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .filter(|value| *value > 0 && *value <= loopal_protocol::MAX_WORKFLOW_OUTPUT_BYTES)
                .ok_or_else(|| anyhow::anyhow!("invalid workflow completion result limit"))?;
            Some(value)
        }
    };
    if workflow_attempt_capability.is_some() != workflow_permission_causation.is_some()
        || workflow_completion_result_limit.is_some() != workflow_permission_causation.is_some()
    {
        anyhow::bail!("workflow authority and completion result limit must be supplied together");
    }

    let start = StartParams {
        cwd: cwd_str,
        model: params["model"].as_str().map(String::from),
        mode: params["mode"].as_str().map(String::from),
        prompt: params["prompt"].as_str().map(String::from),
        permission_mode: params["permission_mode"].as_str().map(String::from),
        decision_mode: params["decision_mode"].as_str().map(String::from),
        no_sandbox,
        sandbox_policy,
        session_id,
        workflow_permission_causation,
        workflow_attempt_capability,
        workflow_completion_result_limit,
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
    bg_spawn_rx: broadcast::Receiver<SpawnNotification>,
    bg_store: Arc<BackgroundTaskStore>,
    frontend: Arc<HubFrontend>,
) -> JoinHandle<Option<AgentOutput>> {
    let (bridge_shutdown_tx, bridge_shutdown_rx) = oneshot::channel();
    let bridge_task =
        crate::bg_task_bridge::spawn(bg_spawn_rx, bg_store, frontend.clone(), bridge_shutdown_rx);
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
        let _ = bridge_shutdown_tx.send(());
        stop_background_bridge(bridge_task, bridge_abort).await;
        task_bridge_abort.abort();
        cron_bridge_abort.abort();
        result.ok().flatten()
    })
}

// Graceful first: let the bridge drain completed events that may still be in
// flight, then abort so a stuck monitor cannot outlive the agent.
async fn stop_background_bridge(bridge_task: JoinHandle<()>, bridge_abort: AbortHandle) {
    if tokio::time::timeout(BRIDGE_SHUTDOWN_GRACE, bridge_task)
        .await
        .is_err()
    {
        bridge_abort.abort();
    }
}

#[cfg(test)]
#[path = "session_spawn/parse_tests.rs"]
mod parse_tests;

#[cfg(test)]
#[path = "session_spawn/parse_completion_limit_tests.rs"]
mod parse_completion_limit_tests;

#[cfg(test)]
#[path = "session_spawn/bridge_shutdown_tests.rs"]
mod bridge_shutdown_tests;
