use std::path::PathBuf;
use std::sync::Arc;

use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use loopal_agent_client::{AgentClient, AgentProcess};
use loopal_protocol::{AgentEvent, AgentEventPayload};

use crate::bridge::{bridge_child_events, read_child_server_info};
use crate::config::AgentConfig;
use crate::registry::AgentHandle;
use crate::shared::AgentShared;
use crate::types::AgentId;

/// Parameters for spawning a new sub-agent.
pub struct SpawnParams {
    pub name: String,
    pub prompt: String,
    pub agent_config: AgentConfig,
    pub parent_model: String,
    pub parent_cancel_token: Option<CancellationToken>,
    /// Override the working directory (e.g. for worktree isolation).
    pub cwd_override: Option<PathBuf>,
    /// Worktree to clean up when the agent finishes (auto-removed if no changes).
    pub worktree: Option<(loopal_git::WorktreeInfo, PathBuf)>,
}

/// Spawn result returned to the caller.
pub struct SpawnResult {
    pub agent_id: AgentId,
    pub handle: AgentHandle,
    /// Receives the sub-agent's final output (Ok) or error (Err) when it completes.
    pub result_rx: tokio::sync::oneshot::Receiver<Result<String, String>>,
}

/// Spawn a sub-agent as a child process (`loopal --serve`).
/// Events are NOT forwarded — TUI connects directly to child's TCP port.
pub async fn spawn_agent(
    shared: &Arc<AgentShared>,
    params: SpawnParams,
) -> Result<SpawnResult, String> {
    let agent_id = uuid::Uuid::new_v4().to_string();
    let model = params
        .agent_config
        .model
        .clone()
        .unwrap_or_else(|| params.parent_model.clone());

    let cancel_token = match params.parent_cancel_token {
        Some(ref parent) => parent.child_token(),
        None => CancellationToken::new(),
    };

    let parent_event_tx = shared
        .parent_event_tx
        .clone()
        .ok_or_else(|| "parent_event_tx not set — cannot spawn sub-agent".to_string())?;

    // Spawn child process
    let agent_proc = AgentProcess::spawn(None)
        .await
        .map_err(|e| format!("failed to spawn agent process: {e}"))?;
    let transport = agent_proc.transport();

    // Initialize IPC connection
    let client = AgentClient::new(transport);
    if let Err(e) = client.initialize().await {
        let _ = agent_proc.shutdown().await;
        return Err(format!("IPC initialize failed: {e}"));
    }

    // Read child's TCP server info for TUI direct connection.
    let child_pid = agent_proc.pid().unwrap_or(0);
    let server_tcp = read_child_server_info(child_pid);

    // Start agent loop in child process — shutdown process on error
    let effective_cwd = params.cwd_override.as_deref().unwrap_or(&shared.cwd);
    let mode = Some("act");
    if let Err(e) = client
        .start_agent(
            effective_cwd,
            Some(&model),
            mode,
            Some(&params.prompt),
            None,
            false,
            None,
        )
        .await
    {
        let _ = agent_proc.shutdown().await;
        return Err(format!("agent/start failed: {e}"));
    }

    // Notify TUI about the sub-agent so it can connect directly via TCP.
    if let Some((port, token)) = server_tcp {
        info!(agent = %params.name, port, "emitting SubAgentSpawned");
        let event = AgentEvent::root(AgentEventPayload::SubAgentSpawned {
            name: params.name.clone(),
            pid: child_pid,
            port,
            token,
        });
        let _ = parent_event_tx.send(event).await;
    }

    // Event bridge: track completion + collect result (TUI gets events via TCP).
    let (result_tx, result_rx) = tokio::sync::oneshot::channel();
    let agent_name = params.name.clone();
    let cleanup_name = params.name.clone();
    let reg = Arc::clone(&shared.registry);
    let router = Arc::clone(&shared.router);
    let proc_handle = Arc::new(tokio::sync::Mutex::new(Some(agent_proc)));

    let join_handle = tokio::spawn({
        let span = info_span!("agent-process", agent = %agent_name);
        let token = cancel_token.clone();
        let proc_for_cleanup = proc_handle.clone();
        let wt = params.worktree;
        async move {
            info!(agent = %agent_name, "sub-agent process started");
            let result = bridge_child_events(client, &parent_event_tx, &agent_name, &token).await;
            let _ = result_tx.send(result);
            // Graceful process shutdown before registry cleanup
            if let Some(proc) = proc_for_cleanup.lock().await.take() {
                let _ = proc.shutdown().await;
            }
            router.unregister(&cleanup_name).await;
            reg.lock().await.remove(&cleanup_name);
            // Clean up worktree after agent finishes (tracked, not fire-and-forget).
            // Uses spawn_blocking to avoid blocking the Tokio worker thread.
            if let Some((info, root)) = wt {
                let _ =
                    tokio::task::spawn_blocking(move || loopal_git::cleanup_if_clean(&root, &info))
                        .await;
            }
            info!(agent = %agent_name, "sub-agent process cleaned up");
        }
        .instrument(span)
    });

    Ok(SpawnResult {
        agent_id: agent_id.clone(),
        handle: AgentHandle {
            id: agent_id,
            name: params.name,
            agent_type: params.agent_config.name.clone(),
            cancel_token,
            join_handle,
            process: proc_handle,
        },
        result_rx,
    })
}
