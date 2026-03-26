//! Internal agent loop setup — builds `AgentLoopParams` from resolved config.

use std::sync::Arc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_context::system_prompt::build_system_prompt;
use loopal_context::{ContextBudget, ContextStore};
use loopal_ipc::connection::{Connection, Incoming};
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_runtime::AgentLoopParams;
use loopal_tool_api::MemoryChannel;

use crate::ipc_frontend::IpcFrontend;
use crate::params::StartParams;

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_inner(
    cwd: &std::path::Path,
    config: &ResolvedConfig,
    start: &StartParams,
    connection: &Arc<Connection>,
    incoming_rx: tokio::sync::mpsc::Receiver<Incoming>,
    kernel: Arc<Kernel>,
    session_dir_override: Option<&std::path::Path>,
    interactive: bool,
) -> anyhow::Result<AgentLoopParams> {
    let model = config.settings.model.clone();
    let compact_model = config.settings.compact_model.clone();
    let max_turns = config.settings.max_turns;
    let permission_mode = config.settings.permission_mode;
    let thinking_config = config.settings.thinking.clone();
    let mode = match start.mode.as_deref() {
        Some("plan") => loopal_runtime::AgentMode::Plan,
        _ => loopal_runtime::AgentMode::Act,
    };
    let mode_str = if mode == loopal_runtime::AgentMode::Plan {
        "plan"
    } else {
        "act"
    };

    let session_manager = if let Some(dir) = session_dir_override {
        loopal_runtime::SessionManager::with_base_dir(dir.to_path_buf())
    } else {
        loopal_runtime::SessionManager::new()?
    };
    let session = session_manager.create_session(cwd, &model)?;

    let interrupt = InterruptSignal::new();
    let interrupt_tx = Arc::new(tokio::sync::watch::channel(0u64).0);

    // Filter interrupt notifications out of the incoming stream so they are
    // processed immediately — even while the agent loop is executing tools.
    let filtered_rx =
        crate::interrupt_filter::spawn(incoming_rx, interrupt.clone(), interrupt_tx.clone());

    let frontend = Arc::new(IpcFrontend::new(connection.clone(), filtered_rx, None));

    // Event channel for sub-agents: events forwarded to TUI via IPC
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<loopal_protocol::AgentEvent>(256);
    let event_conn = connection.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if let Ok(params) = serde_json::to_value(&event) {
                let _ = event_conn
                    .send_notification(loopal_ipc::protocol::methods::AGENT_EVENT.name, params)
                    .await;
            }
        }
    });

    let (observation_tx, _) = tokio::sync::mpsc::channel(256);
    let router = Arc::new(MessageRouter::new(observation_tx));
    let tasks_dir = loopal_config::session_tasks_dir(&session.id)
        .unwrap_or_else(|_| std::env::temp_dir().join("loopal/tasks"));

    let agent_shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        registry: Arc::new(tokio::sync::Mutex::new(AgentRegistry::new())),
        task_store: Arc::new(TaskStore::new(tasks_dir)),
        router,
        cwd: cwd.to_path_buf(),
        depth: 0,
        max_depth: 3,
        agent_name: "main".into(),
        parent_event_tx: Some(event_tx),
        cancel_token: None,
    });

    // Memory observer — only for interactive (root) agents, not sub-agents.
    // Sub-agent processes (non-interactive) skip memory to avoid recursive spawning.
    let memory_enabled = interactive && config.settings.memory.enabled;
    let memory_channel: Option<Arc<dyn MemoryChannel>> = if memory_enabled {
        let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);
        let processor = Arc::new(crate::memory_adapter::ServerMemoryProcessor::new(
            agent_shared.clone(),
            model.clone(),
        ));
        tokio::spawn(loopal_memory::MemoryObserver::new(rx, processor).run());
        Some(Arc::new(crate::memory_adapter::ServerMemoryChannel(tx)))
    } else {
        None
    };

    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(agent_shared);

    let skills: Vec<_> = config.skills.values().map(|e| e.skill.clone()).collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let tool_defs = kernel.tool_definitions();
    let system_prompt = build_system_prompt(
        &config.instructions,
        &tool_defs,
        mode_str,
        &cwd.to_string_lossy(),
        &skills_summary,
        &config.memory,
    );
    let messages = if let Some(prompt) = &start.prompt {
        vec![loopal_message::Message::user(prompt)]
    } else {
        Vec::new()
    };

    let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
    let budget = loopal_runtime::build_initial_budget(
        &model,
        config.settings.max_context_tokens,
        &system_prompt,
        tool_tokens,
    );

    Ok(AgentLoopParams {
        config: loopal_runtime::AgentConfig {
            model,
            compact_model,
            system_prompt,
            mode,
            permission_mode,
            max_turns,
            tool_filter: None,
            interactive,
            thinking_config,
            context_tokens_cap: config.settings.max_context_tokens,
        },
        deps: loopal_runtime::AgentDeps {
            kernel,
            frontend,
            session_manager,
        },
        session,
        store: ContextStore::from_messages(messages, budget),
        interrupt: loopal_runtime::InterruptHandle {
            signal: interrupt,
            tx: interrupt_tx,
        },
        shared: Some(shared_any),
        memory_channel,
    })
}
