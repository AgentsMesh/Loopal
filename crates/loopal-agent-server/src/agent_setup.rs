//! Internal agent loop setup — builds `AgentLoopParams` from resolved config.
use crate::params::{AgentSetupResult, StartParams};
use loopal_agent::shared::{AgentShared, SchedulerHandle};
use loopal_agent::task_store::TaskStore;
use loopal_config::ResolvedConfig;
use loopal_context::system_prompt::build_system_prompt;
use loopal_context::{ContextBudget, ContextStore};
use loopal_kernel::Kernel;
use loopal_protocol::InterruptSignal;
use loopal_runtime::AgentLoopParams;
use loopal_runtime::frontend::traits::AgentFrontend;
use std::sync::Arc;

/// Build `AgentLoopParams` with a pre-constructed frontend.
#[allow(clippy::too_many_arguments)]
pub fn build_with_frontend(
    cwd: &std::path::Path,
    config: &ResolvedConfig,
    start: &StartParams,
    frontend: Arc<dyn AgentFrontend>,
    interrupt: InterruptSignal,
    interrupt_tx: Arc<tokio::sync::watch::Sender<u64>>,
    kernel: Arc<Kernel>,
    hub_connection: Arc<loopal_ipc::connection::Connection>,
    session_dir_override: Option<&std::path::Path>,
) -> anyhow::Result<AgentSetupResult> {
    let router = loopal_provider_api::ModelRouter::from_parts(
        config.settings.model.clone(),
        config.settings.model_routing.clone(),
    );
    let model = router
        .resolve(loopal_provider_api::TaskType::Default)
        .to_string();
    let permission_mode = config.settings.permission_mode;
    let thinking_config = config.settings.thinking.clone();
    let (mode, mode_str) = match start.mode.as_deref() {
        Some("plan") => (loopal_runtime::AgentMode::Plan, "plan"),
        _ => (loopal_runtime::AgentMode::Act, "act"),
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

    // Sub-agent lifecycle events: forward SubAgentSpawned to frontend.
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
    let tasks_dir = loopal_config::session_tasks_dir(&session.id)
        .unwrap_or_else(|_| std::env::temp_dir().join("loopal/tasks"));
    let task_store = Arc::new(TaskStore::new(tasks_dir));
    let (scheduler_handle, scheduled_rx) = SchedulerHandle::create();
    let message_snapshot = Arc::new(std::sync::RwLock::new(Vec::new()));
    let agent_shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        task_store: task_store.clone(),
        hub_connection,
        cwd: cwd.to_path_buf(),
        depth: start.depth.unwrap_or(0),
        agent_name: "main".into(),
        parent_event_tx: Some(event_tx),
        cancel_token: None,
        scheduler_handle,
        message_snapshot: message_snapshot.clone(),
    });

    let memory_channel = crate::memory_adapter::build_memory_channel(
        start.lifecycle == loopal_runtime::LifecycleMode::Persistent,
        &config.settings,
        &agent_shared,
        &model,
    );

    let auto_classifier = if permission_mode == loopal_tool_api::PermissionMode::Auto {
        Some(Arc::new(
            loopal_auto_mode::AutoClassifier::new_with_thresholds(
                config.instructions.clone(),
                cwd.to_string_lossy().into_owned(),
                config.settings.harness.cb_max_consecutive_denials,
                config.settings.harness.cb_max_total_denials,
            ),
        ))
    } else {
        None
    };
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(agent_shared);
    let skills: Vec<_> = config.skills.values().map(|e| e.skill.clone()).collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let tool_defs = kernel.tool_definitions();

    let mut features = Vec::new();
    if config.settings.memory.enabled && memory_channel.is_some() {
        features.push("memory".into());
    }
    if !config.settings.hooks.is_empty() {
        features.push("hooks".into());
    }
    features.push("subagent".into());
    if !config.settings.output_style.is_empty() {
        features.push(format!("style_{}", config.settings.output_style));
    }

    let mut system_prompt = build_system_prompt(
        &config.instructions,
        &tool_defs,
        mode_str,
        &cwd.to_string_lossy(),
        &skills_summary,
        &config.memory,
        start.agent_type.as_deref(),
        features,
        start.depth.unwrap_or(0),
    );
    crate::prompt_post::append_runtime_sections(&mut system_prompt, &kernel);

    let mut messages = resume_messages;
    let mut has_fork = false;
    if let Some(ref fc_value) = start.fork_context
        && start.resume.is_none()
    {
        match serde_json::from_value::<Vec<loopal_message::Message>>(fc_value.clone()) {
            Ok(fork_msgs) => {
                messages.extend(fork_msgs);
                has_fork = true;
            }
            Err(e) => tracing::warn!("fork context deserialization failed, skipping: {e}"),
        }
    }
    if let Some(prompt) = &start.prompt {
        let text = if has_fork {
            format!("{}\n\n{prompt}", loopal_context::fork::FORK_BOILERPLATE)
        } else {
            prompt.to_string()
        };
        messages.push(loopal_message::Message::user(&text));
    }

    let tool_tokens = ContextBudget::estimate_tool_tokens(&tool_defs);
    let budget = loopal_runtime::build_initial_budget(
        &model,
        config.settings.max_context_tokens,
        &system_prompt,
        tool_tokens,
    );
    let lifecycle = start.lifecycle;
    let depth = start.depth.unwrap_or(0);
    let tool_filter = crate::spawn_policy::build_depth_tool_filter(
        &kernel,
        depth,
        config.settings.harness.agent_max_depth,
    );

    let params = AgentLoopParams {
        config: loopal_runtime::AgentConfig {
            lifecycle,
            router,
            system_prompt,
            mode,
            permission_mode,
            tool_filter,
            thinking_config,
            context_tokens_cap: config.settings.max_context_tokens,
            plan_state: None,
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
        scheduled_rx: Some(scheduled_rx),
        auto_classifier,
        harness: config.settings.harness.clone(),
        rewake_rx: None,
        message_snapshot: Some(message_snapshot),
    };
    Ok(AgentSetupResult { params, task_store })
}
