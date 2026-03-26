use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use loopal_agent_client::{AgentClient, AgentClientEvent, AgentProcess};
use loopal_protocol::AgentEvent;

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
    pub cwd_override: Option<std::path::PathBuf>,
}

/// Spawn result returned to the caller.
pub struct SpawnResult {
    pub agent_id: AgentId,
    pub handle: AgentHandle,
    /// Receives the sub-agent's final output (Ok) or error (Err) when it completes.
    pub result_rx: tokio::sync::oneshot::Receiver<Result<String, String>>,
}

/// Spawn a sub-agent as an independent child process (`loopal --serve`).
///
/// The child process gets its own Kernel, tools, and LLM provider.
/// Events are forwarded from the child's IPC to `parent_event_tx`.
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

    // Initialize IPC connection — shutdown process on error
    let client = AgentClient::new(transport);
    if let Err(e) = client.initialize().await {
        let _ = agent_proc.shutdown().await;
        return Err(format!("IPC initialize failed: {e}"));
    }

    // Start agent loop in child process — shutdown process on error
    let effective_cwd = params.cwd_override.as_deref().unwrap_or(&shared.cwd);
    let mode = Some("act");
    if let Err(e) = client
        .start_agent(effective_cwd, Some(&model), mode, Some(&params.prompt), None, false)
        .await
    {
        let _ = agent_proc.shutdown().await;
        return Err(format!("agent/start failed: {e}"));
    }

    // Event bridge: child IPC → parent_event_tx
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

/// Bridge events from child IPC to parent's event channel.
/// Returns the final result text or error when the child finishes.
async fn bridge_child_events(
    mut client: AgentClient,
    parent_tx: &mpsc::Sender<AgentEvent>,
    agent_name: &str,
    cancel_token: &CancellationToken,
) -> Result<String, String> {
    let mut last_text = String::new();
    loop {
        tokio::select! {
            event = client.recv() => {
                match event {
                    Some(AgentClientEvent::AgentEvent(mut ev)) => {
                        // Tag with sub-agent name for TUI display
                        if ev.agent_name.is_none() {
                            ev.agent_name = Some(agent_name.to_string());
                        }
                        // Capture last streamed text as result
                        if let loopal_protocol::AgentEventPayload::Stream { ref text } = ev.payload {
                            last_text.push_str(text);
                        }
                        let _ = parent_tx.send(ev).await;
                    }
                    Some(AgentClientEvent::PermissionRequest { id, .. }) => {
                        // Sub-agents auto-deny permissions
                        let _ = client.respond_permission(id, false).await;
                    }
                    Some(AgentClientEvent::QuestionRequest { id, .. }) => {
                        // Sub-agents auto-cancel questions
                        let resp = loopal_protocol::UserQuestionResponse {
                            answers: vec!["(sub-agent: auto-cancelled)".into()],
                        };
                        let _ = client.respond_question(id, &resp).await;
                    }
                    None => {
                        // IPC disconnected — child exited
                        break;
                    }
                }
            }
            () = cancel_token.cancelled() => {
                let _ = client.shutdown().await;
                break;
            }
        }
    }
    if last_text.is_empty() {
        Ok("(sub-agent completed)".into())
    } else {
        Ok(last_text)
    }
}
