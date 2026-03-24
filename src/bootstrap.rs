use std::sync::Arc;

use clap::Parser;
use tokio::sync::mpsc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::load_config;
use loopal_context::ContextPipeline;
use loopal_context::system_prompt::build_system_prompt;
use loopal_kernel::Kernel;
use loopal_memory::MemoryObserver;
use loopal_protocol::{
    AgentEvent, ControlCommand, Envelope, InterruptSignal, UserQuestionResponse,
};
use loopal_runtime::frontend::question_handler::TuiQuestionHandler;
use loopal_runtime::frontend::tui_permission::TuiPermissionHandler;
use loopal_runtime::projection::project_messages;
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend, agent_loop};
use loopal_session::SessionController;
use loopal_tool_api::MemoryChannel;
use loopal_tui::command::merge_commands;

use crate::cli::Cli;
use crate::memory_adapter::{AgentMemoryProcessor, MpscMemoryChannel};

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    loopal_config::housekeeping::startup_cleanup();
    if let Some(repo_root) = loopal_git::repo_root(&cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }

    let mut config = load_config(&cwd)?;
    cli.apply_overrides(&mut config.settings);

    if cli.acp {
        return loopal_acp::run_acp(config, cwd).await;
    }

    let model = config.settings.model.clone();
    let compact_model = config.settings.compact_model.clone();
    let max_turns = config.settings.max_turns;
    let permission_mode = config.settings.permission_mode;
    let thinking_config = config.settings.thinking.clone();
    let memory_enabled = config.settings.memory.enabled;
    let mode = if cli.plan {
        AgentMode::Plan
    } else {
        AgentMode::Act
    };
    let mode_str = if cli.plan { "plan" } else { "act" }.to_string();

    tracing::info!(model = %model, mode = %mode_str, "starting");

    let mut kernel = Kernel::new(config.settings)?;
    kernel.start_mcp().await?;
    loopal_agent::tools::register_all(&mut kernel);
    let kernel = Arc::new(kernel);

    let session_manager = SessionManager::new()?;
    let (session, mut messages) = if let Some(ref session_id) = cli.resume {
        session_manager.resume_session(session_id)?
    } else {
        (session_manager.create_session(&cwd, &model)?, Vec::new())
    };
    if !cli.prompt.is_empty() {
        messages.push(loopal_message::Message::user(&cli.prompt.join(" ")));
    }

    let (agent_event_tx, agent_event_rx) = mpsc::channel::<AgentEvent>(256);
    let (permission_tx, permission_rx) = mpsc::channel::<bool>(16);
    let (question_tx, question_rx) = mpsc::channel::<UserQuestionResponse>(16);
    let router = Arc::new(MessageRouter::new(agent_event_tx.clone()));
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    router
        .register("main", mailbox_tx)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);
    let interrupt = InterruptSignal::new();
    let interrupt_tx = Arc::new(tokio::sync::watch::channel(0u64).0);

    let frontend = Arc::new(UnifiedFrontend::new(
        None,
        agent_event_tx.clone(),
        mailbox_rx,
        control_rx,
        None,
        Box::new(TuiPermissionHandler::new(
            agent_event_tx.clone(),
            permission_rx,
        )),
        Box::new(TuiQuestionHandler::new(agent_event_tx.clone(), question_rx)),
    ));

    let tasks_dir = loopal_config::session_tasks_dir(&session.id)
        .unwrap_or_else(|_| std::env::temp_dir().join("loopal/tasks"));
    let agent_shared = Arc::new(AgentShared {
        kernel: kernel.clone(),
        registry: Arc::new(tokio::sync::Mutex::new(AgentRegistry::new())),
        task_store: Arc::new(TaskStore::new(tasks_dir)),
        router: router.clone(),
        cwd: cwd.clone(),
        depth: 0,
        max_depth: 3,
        agent_name: "main".to_string(),
        parent_event_tx: Some(agent_event_tx.clone()),
        cancel_token: None,
        worktree_state: Default::default(),
    });

    // Memory observer sidebar — uses AgentShared to spawn memory-maintainer agents
    let memory_channel: Option<Arc<dyn MemoryChannel>> = if memory_enabled {
        let (tx, rx) = mpsc::channel::<String>(64);
        let processor = Arc::new(AgentMemoryProcessor::new(
            agent_shared.clone(),
            model.clone(),
        ));
        tokio::spawn(MemoryObserver::new(rx, processor).run());
        Some(Arc::new(MpscMemoryChannel(tx)))
    } else {
        None
    };

    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(agent_shared);

    let skills: Vec<_> = config.skills.into_values().map(|e| e.skill).collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let commands = merge_commands(&skills);
    let tool_defs = kernel.tool_definitions();
    let system_prompt = build_system_prompt(
        &config.instructions,
        &tool_defs,
        "",
        &cwd.to_string_lossy(),
        &skills_summary,
        &config.memory,
    );

    let session_ctrl = SessionController::new(
        model.clone(),
        mode_str,
        control_tx,
        permission_tx,
        question_tx,
        interrupt.clone(),
        interrupt_tx.clone(),
    );
    if cli.resume.is_some() {
        session_ctrl.load_display_history(project_messages(&messages));
    } else {
        let display_path = abbreviate_home(&cwd);
        session_ctrl.push_welcome(&model, &display_path);
    }

    // Context pipeline: empty — compaction is now a persistent lifecycle event
    // (check_and_compact), and per-message truncation is in prepare_llm_context.
    // The pipeline remains as an extension point for future middleware.
    let context_pipeline = ContextPipeline::new();

    let agent_params = AgentLoopParams {
        kernel,
        session,
        messages,
        model,
        compact_model,
        system_prompt,
        mode,
        permission_mode,
        max_turns,
        frontend,
        session_manager,
        context_pipeline,
        tool_filter: None,
        shared: Some(shared_any),
        interactive: true,
        thinking_config,
        interrupt,
        interrupt_tx,
        memory_channel,
    };

    tokio::spawn(async move {
        if let Err(e) = agent_loop(agent_params).await {
            tracing::error!(error = %e, "agent loop error");
        }
    });

    loopal_tui::run_tui(
        session_ctrl,
        router,
        "main".to_string(),
        commands,
        cwd,
        agent_event_rx,
    )
    .await?;
    tracing::info!("shutting down");
    Ok(())
}

/// Replace the home directory prefix with `~` for compact display.
fn abbreviate_home(path: &std::path::Path) -> String {
    if let Some(home) = dirs::home_dir()
        && let Ok(rel) = path.strip_prefix(&home)
    {
        return format!("~/{}", rel.display());
    }
    path.display().to_string()
}
