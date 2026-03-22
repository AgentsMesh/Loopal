use std::sync::Arc;

use clap::Parser;
use tokio::sync::mpsc;

use loopal_agent::registry::AgentRegistry;
use loopal_agent::router::MessageRouter;
use loopal_agent::shared::AgentShared;
use loopal_agent::task_store::TaskStore;
use loopal_config::load_config;
use loopal_context::ContextPipeline;
use loopal_context::middleware::{ContextGuard, MessageSizeGuard, SmartCompact};
use loopal_context::system_prompt::build_system_prompt;
use loopal_kernel::Kernel;
use loopal_runtime::{AgentLoopParams, AgentMode, SessionManager, UnifiedFrontend, agent_loop};
use loopal_runtime::frontend::tui_permission::TuiPermissionHandler;
use loopal_runtime::frontend::question_handler::TuiQuestionHandler;
use loopal_runtime::projection::project_messages;
use loopal_protocol::UserQuestionResponse;
use loopal_protocol::InterruptSignal;
use loopal_session::SessionController;
use loopal_tui::command::merge_commands;
use loopal_protocol::ControlCommand;
use loopal_protocol::Envelope;
use loopal_protocol::AgentEvent;

use crate::cli::Cli;

pub async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;

    // Ensure directories exist and clean up expired volatile files
    loopal_config::housekeeping::startup_cleanup();

    // Clean up orphaned worktrees from previous crashed sessions
    if let Some(repo_root) = loopal_git::repo_root(&cwd) {
        loopal_git::cleanup_stale_worktrees(&repo_root);
    }

    let mut config = load_config(&cwd)?;
    apply_cli_overrides(&cli, &mut config.settings);

    // ACP mode — replace TUI with JSON-RPC server
    if cli.acp {
        return loopal_acp::run_acp(config, cwd).await;
    }

    let model = config.settings.model.clone();
    let max_turns = config.settings.max_turns;
    let permission_mode = config.settings.permission_mode;
    let thinking_config = config.settings.thinking.clone();
    let mode = if cli.plan { AgentMode::Plan } else { AgentMode::Act };
    let mode_str = if cli.plan { "plan" } else { "act" }.to_string();

    tracing::info!(model = %model, mode = %mode_str, "starting");

    // Build kernel — register agent tools before wrapping in Arc
    let mut kernel = Kernel::new(config.settings)?;
    kernel.start_mcp().await?;
    loopal_agent::tools::register_all(&mut kernel);
    let kernel = Arc::new(kernel);

    // Session management
    let session_manager = SessionManager::new()?;
    let (session, mut messages) = if let Some(ref session_id) = cli.resume {
        session_manager.resume_session(session_id)?
    } else {
        (session_manager.create_session(&cwd, &model)?, Vec::new())
    };

    if !cli.prompt.is_empty() {
        messages.push(loopal_message::Message::user(&cli.prompt.join(" ")));
    }

    // Observation channel — runtime → TUI
    let (agent_event_tx, agent_event_rx) = mpsc::channel::<AgentEvent>(256);

    // Permission channel — TUI → runtime
    let (permission_tx, permission_rx) = mpsc::channel::<bool>(16);

    // Question channel — TUI → runtime (AskUser tool)
    let (question_tx, question_rx) = mpsc::channel::<UserQuestionResponse>(16);

    // MessageRouter — unified data plane
    let router = Arc::new(MessageRouter::new(agent_event_tx.clone()));

    // Root agent mailbox — registered with router
    let (mailbox_tx, mailbox_rx) = mpsc::channel::<Envelope>(16);
    router.register("main", mailbox_tx).await.map_err(|e| anyhow::anyhow!(e))?;

    // Control channel — TUI → runtime (mode switch, clear, compact, model switch)
    let (control_tx, control_rx) = mpsc::channel::<ControlCommand>(16);

    // Shared interrupt signal + async wakeup — TUI sets on ESC / message-while-busy
    let interrupt = InterruptSignal::new();
    let interrupt_notify = Arc::new(tokio::sync::Notify::new());

    // Build UnifiedFrontend (root agent: no cancel_token, TUI permission handler)
    let frontend = Arc::new(UnifiedFrontend::new(
        None, // root agent
        agent_event_tx.clone(),
        mailbox_rx,
        control_rx,
        None, // TUI-controlled lifecycle
        Box::new(TuiPermissionHandler::new(agent_event_tx.clone(), permission_rx)),
        Box::new(TuiQuestionHandler::new(agent_event_tx.clone(), question_rx)),
    ));

    // Build shared agent state (homogeneous with sub-agents)
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
    let shared_any: Arc<dyn std::any::Any + Send + Sync> = Arc::new(agent_shared);

    // Build system prompt from resolved config
    let skills: Vec<_> = config.skills.into_values().map(|e| e.skill).collect();
    let skills_summary = loopal_config::format_skills_summary(&skills);
    let commands = merge_commands(&skills);
    let tool_defs = kernel.tool_definitions();
    let system_prompt = build_system_prompt(
        &config.instructions, &tool_defs, "", &cwd.to_string_lossy(), &skills_summary, &config.memory,
    );

    let session_ctrl = SessionController::new(
        model.clone(), mode_str, control_tx, permission_tx, question_tx,
        interrupt.clone(), interrupt_notify.clone(),
    );

    // Restore display history when resuming a session
    if cli.resume.is_some() {
        let display = project_messages(&messages);
        session_ctrl.load_display_history(display);
    }

    let mut context_pipeline = ContextPipeline::new();
    context_pipeline.add(Box::new(MessageSizeGuard));
    context_pipeline.add(Box::new(ContextGuard));
    context_pipeline.add(Box::new(SmartCompact::new(10)));

    let agent_params = AgentLoopParams {
        kernel: kernel.clone(),
        session: session.clone(),
        messages,
        model: model.clone(),
        system_prompt,
        mode, permission_mode, max_turns,
        frontend,
        session_manager, context_pipeline,
        tool_filter: None,
        shared: Some(shared_any),
        interactive: true,
        thinking_config,
        interrupt: interrupt.clone(),
        interrupt_notify: interrupt_notify.clone(),
    };

    tokio::spawn(async move {
        if let Err(e) = agent_loop(agent_params).await {
            tracing::error!(error = %e, "agent loop error");
        }
    });

    loopal_tui::run_tui(
        session_ctrl, router, "main".to_string(),
        commands, cwd, agent_event_rx,
    ).await?;
    tracing::info!("shutting down");
    Ok(())
}

fn apply_cli_overrides(cli: &Cli, settings: &mut loopal_config::Settings) {
    if let Some(model) = &cli.model {
        settings.model = model.clone();
    }
    if let Some(perm) = &cli.permission {
        settings.permission_mode = match perm.as_str() {
            "bypass" | "yolo" => loopal_tool_api::PermissionMode::Bypass,
            _ => loopal_tool_api::PermissionMode::Supervised,
        };
    }
    if cli.no_sandbox {
        settings.sandbox.policy = loopal_config::SandboxPolicy::Disabled;
    }
}
