use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{Instrument, info, info_span};

use loopal_context::{ContextBudget, ContextPipeline, ContextStore};
use loopal_message::Message;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_runtime::frontend::{AutoCancelQuestionHandler, AutoDenyHandler};
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend, agent_loop};

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

/// Spawn a homogeneous sub-agent as an independent tokio task.
///
/// The child shares the parent's kernel, registry, task_store, and router
/// (with `depth + 1`), so it can recursively spawn further sub-agents.
/// Sub-agent events are forwarded directly to `parent_event_tx` via
/// `UnifiedFrontend` (best-effort, no intermediate channel).
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

    let tool_filter = params
        .agent_config
        .allowed_tools
        .as_ref()
        .map(|tools| tools.iter().cloned().collect::<HashSet<String>>());

    let cancel_token = match params.parent_cancel_token {
        Some(ref parent) => parent.child_token(),
        None => CancellationToken::new(),
    };

    // Create mailbox + control channels for UnifiedFrontend
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    let (_control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);

    // Register with Router so other agents can route envelopes to this agent
    if let Err(e) = shared.router.register(&params.name, mailbox_tx).await {
        return Err(format!("router registration failed: {e}"));
    }

    // Use parent's event_tx directly — no intermediate channel.
    let parent_event_tx = shared
        .parent_event_tx
        .clone()
        .ok_or_else(|| "parent_event_tx not set — cannot spawn sub-agent".to_string())?;

    let frontend = Arc::new(UnifiedFrontend::new(
        Some(params.name.clone()),
        parent_event_tx.clone(),
        mailbox_rx,
        control_rx,
        Some(cancel_token.clone()),
        Box::new(AutoDenyHandler),
        Box::new(AutoCancelQuestionHandler),
    ));

    let session_manager =
        SessionManager::new().map_err(|e| format!("failed to create session manager: {e}"))?;

    let effective_cwd = params.cwd_override.as_deref().unwrap_or(&shared.cwd);
    let session = session_manager
        .create_session(effective_cwd, &model)
        .map_err(|e| format!("failed to create session: {e}"))?;

    let system_prompt = if params.agent_config.system_prompt.is_empty() {
        // Use fragment-based prompt: try specific agent type, fall back to default-subagent
        let agent_type = if params.agent_config.name.is_empty() {
            "default-subagent"
        } else {
            &params.agent_config.name
        };
        let ctx = loopal_prompt::PromptContext {
            cwd: effective_cwd.display().to_string(),
            agent_name: Some(params.name.clone()),
            agent_type: Some(agent_type.to_string()),
            ..Default::default()
        };
        let frags = loopal_prompt_system::system_fragments();
        let registry = loopal_prompt::FragmentRegistry::new(frags);
        let builder = loopal_prompt::PromptBuilder::new(registry);
        builder.build_agent_prompt(agent_type, &ctx)
    } else {
        params.agent_config.system_prompt.clone()
    };

    let max_turns = params.agent_config.max_turns;
    let pipeline = ContextPipeline::new();

    // Homogeneous: child gets same shared refs with depth+1
    let child_shared = Arc::new(AgentShared {
        kernel: Arc::clone(&shared.kernel),
        registry: Arc::clone(&shared.registry),
        task_store: Arc::clone(&shared.task_store),
        router: Arc::clone(&shared.router),
        cwd: effective_cwd.to_path_buf(),
        depth: shared.depth + 1,
        max_depth: shared.max_depth,
        agent_name: params.name.clone(),
        parent_event_tx: Some(parent_event_tx),
        cancel_token: Some(cancel_token.clone()),
        worktree_state: Default::default(),
    });
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(child_shared);
    let session_id = session.id.clone();

    let budget = ContextBudget::calculate(200_000, &system_prompt, 0, 16_384);

    let agent_params = AgentLoopParams {
        kernel: Arc::clone(&shared.kernel),
        session,
        model,
        system_prompt,
        compact_model: shared.kernel.settings().compact_model.clone(),
        store: ContextStore::from_messages(vec![Message::user(&params.prompt)], budget),
        mode: AgentMode::Act,
        permission_mode: params.agent_config.permission_mode,
        max_turns,
        frontend,
        session_manager,
        tool_filter,
        context_pipeline: pipeline,
        shared: Some(shared_any),
        interactive: false,
        thinking_config: loopal_provider_api::ThinkingConfig::Auto,
        interrupt: loopal_protocol::InterruptSignal::new(),
        interrupt_tx: std::sync::Arc::new(tokio::sync::watch::channel(0u64).0),
        memory_channel: None, // Sub-agents do not get memory channel
    };

    let agent_name = params.name.clone();
    let reg = Arc::clone(&shared.registry);
    let cleanup_router = Arc::clone(&shared.router);
    let cleanup_name = params.name.clone();

    let (result_tx, result_rx) = tokio::sync::oneshot::channel();

    let join_handle = tokio::spawn({
        let span = info_span!("agent", session_id = %session_id, agent = %agent_name);
        async move {
            info!(agent = %agent_name, "sub-agent started");
            let result = agent_loop(agent_params).await;
            let output = match result {
                Ok(agent_output) => Ok(agent_output.result),
                Err(e) => {
                    tracing::error!(agent = %agent_name, error = %e, "sub-agent error");
                    Err(e.to_string())
                }
            };
            let _ = result_tx.send(output);
            cleanup_router.unregister(&cleanup_name).await;
            reg.lock().await.remove(&cleanup_name);
            info!(agent = %agent_name, "sub-agent cleaned up");
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
        },
        result_rx,
    })
}
