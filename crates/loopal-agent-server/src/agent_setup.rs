//! Internal agent loop setup — builds `AgentLoopParams` from resolved config.

use std::sync::Arc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_context::system_prompt::build_system_prompt;
use loopal_context::{ContextBudget, ContextStore};
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_runtime::AgentLoopParams;
use loopal_runtime::frontend::traits::AgentFrontend;
use loopal_tool_api::MemoryChannel;

use crate::params::StartParams;

/// Build `AgentLoopParams` with a pre-constructed frontend (HubFrontend or IpcFrontend).
///
/// The caller provides the frontend and interrupt signal, decoupling agent setup
/// from the connection/transport layer.
#[allow(clippy::too_many_arguments)]
pub fn build_with_frontend(
    cwd: &std::path::Path,
    config: &ResolvedConfig,
    start: &StartParams,
    frontend: Arc<dyn AgentFrontend>,
    interrupt: InterruptSignal,
    interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
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
    let (session, resume_messages) = if let Some(ref sid) = start.resume {
        let (s, msgs) = session_manager.resume_session(sid)?;
        (s, msgs)
    } else {
        (session_manager.create_session(cwd, &model)?, Vec::new())
    };

    // Channel for sub-agent lifecycle events (SubAgentSpawned).
    // Only lifecycle events are forwarded — sub-agent internal events go via TCP.
    let (event_tx, mut event_rx) = tokio::sync::mpsc::channel::<loopal_protocol::AgentEvent>(256);
    let lifecycle_frontend = frontend.clone();
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            if matches!(
                event.payload,
                loopal_protocol::AgentEventPayload::SubAgentSpawned { .. }
            ) {
                let _ = lifecycle_frontend.emit(event.payload).await;
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
    let mut system_prompt = build_system_prompt(
        &config.instructions,
        &tool_defs,
        mode_str,
        &cwd.to_string_lossy(),
        &skills_summary,
        &config.memory,
    );

    // Append MCP server instructions (from initialize handshake).
    let mcp_instructions = kernel.mcp_instructions();
    if !mcp_instructions.is_empty() {
        system_prompt.push_str("\n\n# MCP Server Instructions\n");
        for (server_name, instructions) in mcp_instructions {
            system_prompt.push_str(&format!("\n## {server_name}\n{instructions}\n"));
        }
    }

    // Append MCP resource and prompt summaries so the LLM knows what's available.
    let mcp_resources = kernel.mcp_resources();
    if !mcp_resources.is_empty() {
        system_prompt.push_str("\n\n# Available MCP Resources\n");
        for (server, res) in mcp_resources {
            let desc = res.description.as_deref().unwrap_or("");
            system_prompt.push_str(&format!("\n- `{}` ({server}): {desc}", res.uri));
        }
    }

    let mcp_prompts = kernel.mcp_prompts();
    if !mcp_prompts.is_empty() {
        system_prompt.push_str("\n\n# Available MCP Prompts\n");
        for (server, p) in mcp_prompts {
            let desc = p.description.as_deref().unwrap_or("");
            system_prompt.push_str(&format!("\n- `{}` ({server}): {desc}", p.name));
        }
    }
    let mut messages = resume_messages;
    if let Some(prompt) = &start.prompt {
        messages.push(loopal_message::Message::user(prompt));
    }

    let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
    let budget = loopal_runtime::build_initial_budget(
        &model,
        config.settings.max_context_tokens,
        &system_prompt,
        tool_tokens,
    );

    let params = AgentLoopParams {
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
    };
    Ok(params)
}
